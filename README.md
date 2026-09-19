# felgens
Bilibili live danmu websocket library

[![asciicast](https://asciinema.org/a/zQIlXtbOQCIzlghjDxpaBbcHJ.png)](https://asciinema.org/a/zQIlXtbOQCIzlghjDxpaBbcHJ)


## Usage

```rust
use felgens::{stream, WsStreamMessageType};
use futures_util::StreamExt;

#[tokio::main]
async fn main() {
   // bilibili live room id (true id): 22746343
   // cookie from a logged-in browser (needs SESSDATA; cookie_scoop works well)
   let cookie = std::env::var("FELGENS_COOKIE").unwrap();

   let mut messages = stream(22746343, &cookie).await.unwrap();
   while let Some(message) = messages.next().await {
       match message {
           Ok(WsStreamMessageType::DanmuMsg(danmu)) => println!("{}", danmu.msg),
           Ok(_) => {}
           Err(e) => eprintln!("read error: {e}"),
       }
   }
}
```

Need raw JSON instead of parsed messages? Use `raw_stream` (see `examples/danmu_str.rs`).

Or run `cargo run --example danmu`

## To-do!

- [x] 弹幕
- [x] SC
- [x] xxx 进了该房间
- [ ] 礼物
- [ ] 红包
