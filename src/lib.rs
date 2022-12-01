use std::time::Duration;

use anyhow::{anyhow, Result};
use futures_util::{Future, SinkExt, StreamExt, TryStreamExt};
use serde::Serialize;
use tokio::{sync::mpsc, time::sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};
pub use ws_type::{DanmuMessage, InteractWord, SuperChatMessage, WsStreamMessageType, SendGift};

use crate::{http_client::HttpClient, pack::build_pack};
use ws_type::WsStreamCtx;

mod http_client;
mod pack;
mod ws_type;

type WsReadType = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
>;

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
) -> Result<()> {
    let (mut read, timeout_worker) = prepare(roomid).await?;

    let recv = async {
        while let Ok(Some(msg)) = read.try_next().await {
            let data = msg.into_data();

            if !data.is_empty() {
                let s = build_pack(&data);

                if let Ok(msgs) = s {
                    for i in msgs {
                        let ws = WsStreamCtx::new(&i).unwrap();
                        match ws.match_msg() {
                            Ok(v) => tx.send(v).unwrap(),
                            Err(_) => continue,
                        }
                    }
                }
            }
        }
    };

    tokio::select!(v = timeout_worker => v, v = recv => v);

    Ok(())
}

pub async fn ws_socket_str(tx: mpsc::UnboundedSender<String>, roomid: u64) -> Result<()> {
    let (mut read, timeout_worker) = prepare(roomid).await?;

    let recv = async {
        while let Ok(Some(msg)) = read.try_next().await {
            // dbg!(&msg);
            let data = msg.into_data();

            if !data.is_empty() {
                // let data = pack(&data).unwrap();

                let s = build_pack(&data);

                if let Ok(msgs) = s {
                    for i in msgs {
                        tx.send(i).unwrap();
                    }
                }
            }
        }
    };

    tokio::select!(v = timeout_worker => v, v = recv => v);

    Ok(())
}

async fn prepare(roomid: u64) -> Result<(WsReadType, impl Future<Output = ()>)> {
    let client = HttpClient::new()?;
    let roomid = client.get_room_id(roomid).await?;
    let dammu_info = client.get_dammu_info(roomid).await?.data;
    let key = dammu_info.token;
    let host_list = dammu_info.host_list;
    let mut con = None;

    for i in host_list {
        let host = format!("wss://{}/sub", i.host);
        if let Ok((c, _)) = connect_async(&host).await {
            con = Some(c);
            break;
        }
    }

    let con = con.ok_or_else(|| anyhow!("Can not connect any websocket host!"))?;
    let (mut write, read) = con.split();
    let json = serde_json::to_string(&WsSend { roomid, key })?;
    let json = pack::encode(&json, 7)?;
    write.send(Message::binary(json)).await?;

    let timeout_worker = async move {
        loop {
            write
                .send(Message::binary(pack::encode("", 2).unwrap()))
                .await
                .unwrap();
            // dbg!("send");
            sleep(Duration::from_secs(5)).await;
        }
    };

    Ok((read, timeout_worker))
}
// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }
