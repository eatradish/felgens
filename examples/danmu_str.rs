use felgens::{ws_socket_str, FelgensResult};
use tokio::sync::mpsc::{self, UnboundedReceiver};

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::unbounded_channel();

    // 关注艾露露 (https://live.bilibili.com/22746343) 喵！

    let room_id = std::env::var("FELGENS_ROOMID")
        .ok()
        .and_then(|x| x.parse::<u64>().ok())
        .unwrap_or(22746343);

    let ws = ws_socket_str(tx, room_id);

    if let Err(e) = tokio::select! {v = ws => v, v = recv(rx) => v} {
        eprintln!("{}", e);
    }
}

async fn recv(mut rx: UnboundedReceiver<String>) -> FelgensResult<()> {
    while let Some(msg) = rx.recv().await {
        println!("{}", msg);
    }

    Ok(())
}
