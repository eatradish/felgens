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
    let (write, read) = prepare(roomid, cookie).await?;
    let messages = frame_stream(read)
        .and_then(|message| future::ready(Ok(typed_messages_of(message))))
        .map_ok(|items| stream::iter(items.into_iter().map(Ok)))
        .try_flatten();
    Ok(with_heartbeat(messages, write))
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
    let (write, read) = prepare(roomid, cookie).await?;
    let messages = frame_stream(read)
        .and_then(|message| future::ready(Ok(raw_messages_of(message))))
        .map_ok(|items| stream::iter(items.into_iter().map(Ok)))
        .try_flatten();
    Ok(with_heartbeat(messages, write))
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

async fn prepare(roomid: u64, cookie: &str) -> FelgensResult<(WsWriteType, WsReadType)> {
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
    let key = dammu_info.token;
    let host_list = dammu_info.host_list;
    let mut con = None;

    debug!("ws host list: {:?}", host_list);

    for i in host_list {
        let host = format!("wss://{}/sub", i.host);
        if let Ok((c, _)) = connect_async(&host).await {
            con = Some(c);
            info!("Connected ws host: {}", host);
            break;
        } else {
            warn!("Connect ws host: {} has error, trying next host ...", host);
        }
    }

    let con = con.ok_or_else(|| FelgensError::FailedConnectWsHost)?;
    let (mut write, read) = con.split();

    let json = serde_json::to_string(&WsSend {
        roomid,
        key,
        uid: uid as u32,
    })?;

    debug!("Websocket sending json: {}", json);
    let json = pack::encode(&json, 7);
    write.send(Message::binary(json)).await?;

    Ok((write, read))
}

async fn send_heartbeat_packets(mut write: WsWriteType) -> FelgensResult<()> {
    loop {
        write.send(Message::binary(pack::encode("", 2))).await?;
        debug!("Heartbeat packets have been sent!");
        sleep(Duration::from_secs(30)).await;
    }
}
