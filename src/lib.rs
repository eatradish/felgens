use std::time::Duration;

use futures_util::{future, stream, SinkExt, Stream, StreamExt, TryStreamExt};
use reqwest::header::HeaderMap;
use serde::Serialize;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};
pub use ws_type::{
    DanmuMessage, InteractWord, LiveMessageError, LiveMessageResult, SendGift, SuperChatMessage,
    WsStreamMessageType,
};

use log::{debug, info, warn};

use crate::{http_client::HttpClient, pack::build_pack};
use ws_type::WsStreamCtx;

mod http_client;
mod pack;
mod sign;
mod ws_type;

type WsReadType = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
>;

type WsWriteType = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

#[derive(thiserror::Error, Debug)]
pub enum FelgensError {
    #[error(transparent)]
    UrlError(#[from] url::ParseError),
    #[error("Can not connect any websocket host!")]
    FailedConnectWsHost,
    #[error("弹幕认证被拒（code={code}）：{message}")]
    AuthFailed { code: i64, message: String },
    #[error("弹幕认证超时：没等到服务端的认证回复")]
    AuthTimeout,
    #[error(transparent)]
    SerdeError(#[from] serde_json::Error),
    #[error(transparent)]
    TungsteniteError(#[from] tokio_tungstenite::tungstenite::Error),
    #[error(transparent)]
    LiveMessageError(#[from] Box<LiveMessageError>),
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[error("接口 {what} 返回 code={code}：{message}")]
    ApiError {
        what: String,
        code: i64,
        message: String,
    },
    #[error(transparent)]
    ScrollError(#[from] scroll::Error),
    #[error(transparent)]
    ReadError(#[from] std::io::Error),
    #[error("Unsupport proto version! {0}")]
    UnsupportProto(String),
    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),
}

pub type FelgensResult<T> = Result<T, FelgensError>;

#[derive(Serialize)]
struct WsSend {
    uid: u32,
    roomid: u64,
    key: String,
    // protover: u32,
    // platform: String,
    // clientver: String,
    // #[serde(rename = "type")]
    // t: u32,
}

/// 连上弹幕服务器并完成鉴权，返回解析好的消息流。
///
/// `cookie` 是登录浏览器里的 Cookie（`cookie_scoop` 读出来就行）：取 uid、签名弹幕
/// 口令都要用它。连接/鉴权失败会直接返回 `Err`；连上之后的读取错误以流里的 `Err` 项
/// 出现，服务端正常断开就是流结束（`None`）。流被 drop 时连接和心跳一起停掉。
///
/// 每次调用都会重新取一份凭据（`nav` + `getDanmuInfo`）；想重连不加请求、把凭据
/// 攒在手里反复用，走 [`ticket`] + [`Ticket::stream`]。
///
/// ```no_run
/// use felgens::{stream, WsStreamMessageType};
/// use futures_util::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let cookie = std::env::var("FELGENS_COOKIE").unwrap();
///     let mut messages = stream(22746343, &cookie).await.unwrap();
///
///     while let Some(message) = messages.next().await {
///         match message {
///             Ok(WsStreamMessageType::DanmuMsg(danmu)) => println!("{}", danmu.msg),
///             Ok(_) => {}
///             Err(e) => eprintln!("read error: {e}"),
///         }
///     }
/// }
/// ```
pub async fn stream(
    roomid: u64,
    cookie: &str,
) -> FelgensResult<impl Stream<Item = FelgensResult<WsStreamMessageType>> + Send> {
    ticket(roomid, cookie).await?.stream().await
}

/// 同 [`stream`]，但每条消息是原始 JSON 字符串。
///
/// 只关心少数几种消息（比如红包广播）想自己解析时用它更省事。
///
/// ```no_run
/// use felgens::raw_stream;
/// use futures_util::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let cookie = std::env::var("FELGENS_COOKIE").unwrap();
///     let mut messages = raw_stream(22746343, &cookie).await.unwrap();
///
///     while let Some(raw) = messages.next().await {
///         match raw {
///             Ok(raw) => println!("{}", raw),
///             Err(e) => eprintln!("read error: {e}"),
///         }
///     }
/// }
/// ```
pub async fn raw_stream(
    roomid: u64,
    cookie: &str,
) -> FelgensResult<impl Stream<Item = FelgensResult<String>> + Send> {
    ticket(roomid, cookie).await?.raw_stream().await
}

/// 一份弹幕连接凭据：**取一次，同一间房的每次重连都能接着用**。
///
/// 弹幕 token 与房间绑定（实测：拿 A 房的 token 去连 B 房会被服务端直接断开），
/// 所以一份凭据只对应一间房；但同一间房里它管得很久——重连、换服务器都不用再走
/// `nav` + `getDanmuInfo`。token 终究会过期，那时认证会报
/// [`FelgensError::AuthFailed`]（比如 code `-101`），丢掉重取一份即可。
#[derive(Clone)]
pub struct Ticket {
    roomid: u64,
    uid: u64,
    token: String,
    hosts: Vec<String>,
}

impl std::fmt::Debug for Ticket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // token 是凭据，别原样落进日志
        f.debug_struct("Ticket")
            .field("roomid", &self.roomid)
            .field("uid", &self.uid)
            .field("token", &"<略>")
            .field("hosts", &self.hosts)
            .finish()
    }
}

impl Ticket {
    /// 凭据对应的真实房间号。
    pub fn roomid(&self) -> u64 {
        self.roomid
    }

    /// 用这份凭据连一次弹幕（不再请求 `nav` / `getDanmuInfo`），返回解析好的消息流。
    pub async fn stream(
        &self,
    ) -> FelgensResult<impl Stream<Item = FelgensResult<WsStreamMessageType>> + Send> {
        let (write, read) = prepare(self).await?;
        let messages = frame_stream(read)
            .and_then(|message| future::ready(Ok(typed_messages_of(message))))
            .map_ok(|items| stream::iter(items.into_iter().map(Ok)))
            .try_flatten();
        Ok(with_heartbeat(messages, write))
    }

    /// 同 [`Ticket::stream`]，但每条消息是原始 JSON 字符串。
    pub async fn raw_stream(
        &self,
    ) -> FelgensResult<impl Stream<Item = FelgensResult<String>> + Send> {
        let (write, read) = prepare(self).await?;
        let messages = frame_stream(read)
            .and_then(|message| future::ready(Ok(raw_messages_of(message))))
            .map_ok(|items| stream::iter(items.into_iter().map(Ok)))
            .try_flatten();
        Ok(with_heartbeat(messages, write))
    }
}

/// 取一份弹幕连接凭据：`nav`（uid + WBI 口令）加 `getDanmuInfo`（token + 服务器列表）。
///
/// 拿到之后靠 [`Ticket`] 反复连接（包括断线重连），不必每次重打这两个接口；
/// 认证被拒时再取一份新的。`cookie` 是登录浏览器里的 Cookie。
///
/// ```no_run
/// use futures_util::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let cookie = std::env::var("FELGENS_COOKIE").unwrap();
///     let ticket = felgens::ticket(22746343, &cookie).await.unwrap();
///     let mut messages = ticket.raw_stream().await.unwrap();
///
///     while let Some(raw) = messages.next().await {
///         println!("{raw:?}");
///     }
/// }
/// ```
pub async fn ticket(roomid: u64, cookie: &str) -> FelgensResult<Ticket> {
    let client = HttpClient::new()?;
    let roomid = client.get_room_id(roomid).await?;

    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::COOKIE,
        cookie.parse().expect("Failed to parse cookie!"),
    );

    let (_, _, uid) = client.get_nav(headers.clone()).await?;
    debug!("uid is: {}", uid);

    let dammu_info = client.get_dammu_info(roomid, headers).await?.data;

    Ok(Ticket {
        roomid,
        uid,
        token: dammu_info.token,
        hosts: dammu_info
            .host_list
            .into_iter()
            .map(|host| host.host)
            .collect(),
    })
}

/// 把 WebSocket 的消息帧流统一成 [`FelgensError`] 错误。
fn frame_stream(read: WsReadType) -> impl Stream<Item = FelgensResult<Message>> + Send {
    read.map_err(FelgensError::from)
}

/// 一帧数据 → 若干条原始 JSON（解不开就一条也不给）。
fn raw_messages_of(message: Message) -> Vec<String> {
    let data = message.into_data();
    if data.is_empty() {
        return Vec::new();
    }
    build_pack(&data).unwrap_or_default()
}

/// 一帧数据 → 若干条解析好的消息；不认识的 cmd 只记 debug，跳过。
fn typed_messages_of(message: Message) -> Vec<WsStreamMessageType> {
    let mut messages = Vec::new();
    for raw in raw_messages_of(message) {
        match WsStreamCtx::new(&raw).and_then(|ctx| ctx.match_msg()) {
            Ok(message) => messages.push(message),
            Err(e) => debug!("skipping message: {e}"),
        }
    }
    messages
}

/// 给消息流挂上后台心跳任务。
///
/// 写半边交给任务每 30 秒打一次心跳；任务句柄藏在流里——流活着任务就活着，
/// 流被 drop 时任务一起停掉。
fn with_heartbeat<S>(stream: S, write: WsWriteType) -> impl Stream<Item = S::Item> + Send
where
    S: Stream + Send,
{
    let heartbeat = Heartbeat(tokio::spawn(async move {
        if let Err(e) = send_heartbeat_packets(write).await {
            debug!("heartbeat task stopped: {e}");
        }
    }));

    stream.map(move |item| {
        // 占住句柄，别让它提前掉了
        let _ = &heartbeat;
        item
    })
}

/// 心跳任务的句柄：被 drop（流结束/丢弃）时把任务一起停掉。
struct Heartbeat(JoinHandle<()>);

impl Drop for Heartbeat {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// 认证回复的等待时限：等不到就当这次连接没成。
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);

async fn prepare(ticket: &Ticket) -> FelgensResult<(WsWriteType, WsReadType)> {
    let mut con = None;

    debug!("ws host list: {:?}", ticket.hosts);

    for host in &ticket.hosts {
        let url = format!("wss://{host}/sub");
        if let Ok((c, _)) = connect_async(&url).await {
            con = Some(c);
            info!("Connected ws host: {url}");
            break;
        } else {
            warn!("Connect ws host: {url} has error, trying next host ...");
        }
    }

    let con = con.ok_or_else(|| FelgensError::FailedConnectWsHost)?;
    let (mut write, mut read) = con.split();

    let json = serde_json::to_string(&WsSend {
        roomid: ticket.roomid,
        key: ticket.token.clone(),
        uid: ticket.uid as u32,
    })?;

    debug!("Websocket sending json: {json}");
    let json = pack::encode(&json, 7);
    write.send(Message::binary(json)).await?;

    // 等服务端的认证回复（op=8）：code 非 0（比如 token 过期的 -101）就直接报
    // `AuthFailed`——别让调用方以为连上了、手里攥着一根马上会被掐的线
    check_auth_reply(&mut read).await?;

    Ok((write, read))
}

/// 读帧找认证回复（op=8）并检查 `code`；认证回复之前收到的零碎帧先放掉。
async fn check_auth_reply(read: &mut WsReadType) -> FelgensResult<()> {
    let wait = async {
        loop {
            match read.next().await {
                Some(Ok(message)) if message.is_binary() => {
                    let data = message.into_data();
                    if let Some(body) = pack::auth_reply_body(&data)? {
                        return check_auth_code(body);
                    }
                    debug!("认证回复之前先收到别的帧，先放掉");
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(e.into()),
                None => return Err(FelgensError::AuthTimeout),
            }
        }
    };

    match tokio::time::timeout(AUTH_TIMEOUT, wait).await {
        Ok(result) => result,
        Err(_) => Err(FelgensError::AuthTimeout),
    }
}

/// 认证回复的 JSON：`code` 非 0 视为认证被拒。
fn check_auth_code(body: &str) -> FelgensResult<()> {
    #[derive(serde::Deserialize)]
    struct Reply {
        code: i64,
        #[serde(default)]
        message: Option<String>,
    }

    let reply: Reply = serde_json::from_str(body)?;
    if reply.code == 0 {
        return Ok(());
    }

    Err(FelgensError::AuthFailed {
        code: reply.code,
        message: reply.message.unwrap_or_default(),
    })
}

async fn send_heartbeat_packets(mut write: WsWriteType) -> FelgensResult<()> {
    loop {
        write.send(Message::binary(pack::encode("", 2))).await?;
        debug!("Heartbeat packets have been sent!");
        sleep(Duration::from_secs(30)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_reply_code_is_checked() {
        // 通过：code 0
        let frame = pack::encode(r#"{"code":0}"#, 8);
        let body = pack::auth_reply_body(&frame).unwrap().unwrap();
        assert!(check_auth_code(body).is_ok());

        // token 过期：-101
        let frame = pack::encode(r#"{"code":-101,"message":"token 过期"}"#, 8);
        let body = pack::auth_reply_body(&frame).unwrap().unwrap();
        match check_auth_code(body) {
            Err(FelgensError::AuthFailed { code, message }) => {
                assert_eq!(code, -101);
                assert!(message.contains("过期"));
            }
            other => panic!("应当报 AuthFailed：{other:?}"),
        }

        // 不是认证回复的帧（op=5）不认
        let frame = pack::encode(r#"{"cmd":"DANMU_MSG"}"#, 5);
        assert!(pack::auth_reply_body(&frame).unwrap().is_none());
    }
}
