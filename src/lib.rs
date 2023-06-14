use std::time::Duration;

use futures_util::{SinkExt, StreamExt, TryStreamExt};
use serde::Serialize;
use tokio::{sync::mpsc, time::sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};
pub use ws_type::{
    DanmuMessage, InteractWord, LiveMessageError, LiveMessageResult, SendGift, SuperChatMessage,
    WsStreamMessageType,
};

use log::{debug, error, info, warn};

use crate::{http_client::HttpClient, pack::build_pack};
use ws_type::WsStreamCtx;

mod http_client;
mod pack;
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
    LiveMessageError(#[from] LiveMessageError),
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[error(transparent)]
    ScrollError(#[from] scroll::Error),
    #[error(transparent)]
    ReadError(#[from] std::io::Error),
    #[error("Unsupport proto version! {0}")]
    UnsupportProto(String),
    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error(transparent)]
    WsStreamMessageTypeSendError(#[from] tokio::sync::mpsc::error::SendError<WsStreamMessageType>),
    #[error(transparent)]
    StringSendError(#[from] tokio::sync::mpsc::error::SendError<String>),
}

pub type FelgensResult<T> = Result<T, FelgensError>;

#[derive(Serialize)]
struct WsSend {
    // uid: u32,
    roomid: u64,
    key: String,
    // protover: u32,
    // platform: String,
    // clientver: String,
    // #[serde(rename = "type")]
    // t: u32,
}

/// Init Bilibili websocket channel
/// ```rust
/// use anyhow::Result;
/// use bililive::{ws_socket_object, WsStreamMessageType};
/// use tokio::sync::mpsc::{self, UnboundedReceiver};

/// #[tokio::main]
/// async fn main() {
///     let (tx, rx) = mpsc::unbounded_channel();

///     // bilibili live room id: 22746343

///     let ws = ws_socket_object(tx, 5424);

///     if let Err(e) = tokio::select! {v = ws => v, v = recv(rx) => v} {
///         eprintln!("{}", e);
///     }
/// }

/// async fn recv(mut rx: UnboundedReceiver<WsStreamMessageType>) -> Result<()> {
///     while let Some(msg) = rx.recv().await {
///         println!("{:?}", msg);
///     }

///     Ok(())
/// }
/// ```
pub async fn ws_socket_object(
    tx: mpsc::UnboundedSender<WsStreamMessageType>,
    roomid: u64,
) -> FelgensResult<()> {
    let (write, read) = prepare(roomid).await?;

    tokio::select!(v = send_heartbeat_packets(write) => v, v = recv(read, tx) => v)?;

    Ok(())
}

async fn recv(mut read: WsReadType, tx: mpsc::UnboundedSender<WsStreamMessageType>) -> FelgensResult<()> {
    while let Ok(Some(msg)) = read.try_next().await {
        let data = msg.into_data();

        if !data.is_empty() {
            let s = build_pack(&data);

            if let Ok(msgs) = s {
                for i in msgs {
                    let ws = WsStreamCtx::new(&i);
                    if let Ok(ws) = ws {
                        match ws.match_msg() {
                            Ok(v) => tx.send(v)?,
                            Err(e) => {
                                warn!(
                                    "This message parsing is not yet supported:\nMessage: {i}\nErr: {e:#?}"
                                );
                            }
                        }
                    } else {
                        error!("{}", ws.unwrap_err());
                    }
                }
            }
        }
    }

    Ok(())
}

async fn recv_string(mut read: WsReadType, tx: mpsc::UnboundedSender<String>) -> FelgensResult<()> {
    while let Ok(Some(msg)) = read.try_next().await {
        let data = msg.into_data();

        if !data.is_empty() {
            let s = build_pack(&data);

            if let Ok(msgs) = s {
                for i in msgs {
                    tx.send(i)?;
                }
            }
        }
    }

    Ok(())
}

pub async fn ws_socket_str(tx: mpsc::UnboundedSender<String>, roomid: u64) -> FelgensResult<()> {
    let (write, read) = prepare(roomid).await?;

    tokio::select!(v = send_heartbeat_packets(write) => v, v = recv_string(read, tx) => v)?;

    Ok(())
}

async fn prepare(roomid: u64) -> FelgensResult<(WsWriteType, WsReadType)> {
    let client = HttpClient::new()?;
    let roomid = client.get_room_id(roomid).await?;
    let dammu_info = client.get_dammu_info(roomid).await?.data;
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
    let json = serde_json::to_string(&WsSend { roomid, key })?;

    debug!("Websocket sending json: {}", json);
    let json = pack::encode(&json, 7);
    write.send(Message::binary(json)).await?;

    Ok((write, read))
}

async fn send_heartbeat_packets(mut write: WsWriteType) -> FelgensResult<()> {
    loop {
        write
            .send(Message::binary(pack::encode("", 2)))
            .await?;
        debug!("Heartbeat packets have been sent!");
        sleep(Duration::from_secs(30)).await;
    }
}
