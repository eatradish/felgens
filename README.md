# felgens
Bilibili live danmu websocket library

[![asciicast](https://asciinema.org/a/zQIlXtbOQCIzlghjDxpaBbcHJ.png)](https://asciinema.org/a/zQIlXtbOQCIzlghjDxpaBbcHJ)


## Usage

```rust
use felgens::{FelgensResult, WsStreamMessageType, ws_socket};
use tokio::sync::mpsc::{self, UnboundedReceiver};

#[tokio::main]
async fn main() {
   let (tx, rx) = mpsc::unbounded_channel();

   // bilibili live room id (true id): 22746343
   // cookie from a logged-in browser (needs SESSDATA; cookie_scoop works well)
   let cookie = std::env::var("FELGENS_COOKIE").unwrap();
   let ws = ws_socket(tx, 22746343, &cookie);

   if let Err(e) = tokio::select! {v = ws => v, v = recv(rx) => v} {
       eprintln!("{}", e);
   }
}

async fn recv(mut rx: UnboundedReceiver<WsStreamMessageType>) -> FelgensResult<()> {
   while let Some(msg) = rx.recv().await {
       println!("{:?}", msg);
   }

   Ok(())
}
```

Need raw JSON instead of parsed messages? Use `ws_socket_raw` (see `examples/danmu_str.rs`).

Or run `cargo run --example danmu`

## To-do!

- [x] 弹幕
- [x] SC
- [x] xxx 进了该房间
- [ ] 礼物
- [ ] 红包
