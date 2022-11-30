use anyhow::Result;
use felgens::{
    ws_socket_object, DanmuMessage, InteractWord, SuperChatMessage, WsStreamMessageType,
};
use owo_colors::OwoColorize;
use tokio::sync::mpsc::{self, UnboundedReceiver};

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::unbounded_channel();

    // 关注艾露露 (https://live.bilibili.com/22746343) 瞄！

    let ws = ws_socket_object(tx, 22571722);

    if let Err(e) = tokio::select! {v = ws => v, v = recv(rx) => v} {
        eprintln!("{}", e);
    }
}

async fn recv(mut rx: UnboundedReceiver<WsStreamMessageType>) -> Result<()> {
    while let Some(msg) = rx.recv().await {
        match msg {
            WsStreamMessageType::DanmuMsg(msg) => print_danmu_msg(msg),
            WsStreamMessageType::SuperChatMessage(msg) => print_sc(msg),
            WsStreamMessageType::InteractWord(msg) => print_interact_word(msg),
        }
    }

    Ok(())
}

fn print_danmu_msg(msg: DanmuMessage) {
    let mut s = String::new();

    let fl = if let Some(fl) = msg.fan_level {
        s.push_str(&format!(
            "[{}({})] {}: ",
            msg.fan.unwrap_or("".to_string()),
            fl,
            msg.username
        ));

        fl
    } else {
        s.push_str(&format!("{}: ", msg.username));

        0
    };

    s.push_str(&msg.msg);

    s = match fl {
        25..=28 => s.blue().to_string(),
        9..=12 => s.fg_rgb::<119, 150, 154>().to_string(),
        21..=24 => s.cyan().to_string(),
        0 => s,
        1..=8 => s.fg_rgb::<51, 103, 116>().to_string(),
        29.. => s.fg_rgb::<119, 60, 141>().to_string(),
        13..=17 => s.red().to_string(),
        18..=20 => s.yellow().to_string(),
    };

    println!("{}", s);
}

fn print_sc(msg: SuperChatMessage) {
    println!("{} SC ({}): {}", msg.uname, msg.price, msg.msg);
}

fn print_interact_word(msg: InteractWord) {
    let mut s = String::new();

    if let Some(fan) = msg.fan {
        s.push_str(&format!(
            "[{}({})] ",
            fan,
            msg.fan_level.expect("Should exist")
        ));
    }

    s.push_str(&format!("{} 进入了直播间", msg.uname));

    println!("{}", s);
}
