use felgens::raw_stream;
use futures_util::StreamExt;

#[tokio::main]
async fn main() {
    let room_id = std::env::var("FELGENS_ROOMID")
        .ok()
        .and_then(|x| x.parse::<u64>().ok())
        .unwrap_or(22746343);

    let cookie = std::env::var("FELGENS_COOKIE").unwrap();

    let mut messages = match raw_stream(room_id, &cookie).await {
        Ok(messages) => messages,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    while let Some(raw) = messages.next().await {
        match raw {
            Ok(raw) => println!("{}", raw),
            Err(e) => eprintln!("{}", e),
        }
    }
}
