use anyhow::Result;
use felgens::ws_socket_str;
use tokio::sync::mpsc::{self, UnboundedReceiver};

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::unbounded_channel();

    // 关注艾露露 (https://live.bilibili.com/22746343) 喵！

    let ws = ws_socket_str(tx, 22746343);

    if let Err(e) = tokio::select! {v = ws => v, v = recv(rx) => v} {
        eprintln!("{}", e);
    }
}

async fn recv(mut rx: UnboundedReceiver<String>) -> Result<()> {
    while let Some(msg) = rx.recv().await {
        println!("{}", msg);
    }

    Ok(())
}
