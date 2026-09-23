use felgens::send_danmaku;

#[tokio::main]
async fn main() {
    let room_id = std::env::var("FELGENS_ROOMID")
        .ok()
        .and_then(|x| x.parse::<u64>().ok())
        .unwrap_or(22746343);

    let cookie = std::env::var("FELGENS_COOKIE").unwrap();
    let msg = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "hello".to_string());

    match send_danmaku(room_id, &cookie, &msg).await {
        Ok(()) => println!("sent to room {room_id}: {msg}"),
        Err(e) => eprintln!("failed to send: {e}"),
    }
}
