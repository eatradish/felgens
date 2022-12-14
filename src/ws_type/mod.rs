mod dannmu_msg;
mod interact_word;
mod send_gift;
mod super_chat;
mod util;

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;

pub use self::dannmu_msg::DanmuMessage;
pub use self::interact_word::InteractWord;
pub use self::send_gift::SendGift;
pub use self::super_chat::SuperChatMessage;

#[derive(Debug, Deserialize)]
pub struct WsStreamCtx {
    cmd: Option<String>,
    info: Option<Vec<Value>>,
    data: Option<WsStreamCtxData>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WsStreamCtxData {
    message: Option<String>,
    price: Option<u32>,
    start_time: Option<u64>,
    time: Option<u32>,
    uid: Option<Value>,
    user_info: Option<WsStreamCtxDataUser>,
    medal_info: Option<WsStreamCtxDataMedalInfo>,
    uname: Option<String>,
    fans_medal: Option<WsStreamCtxDataMedalInfo>,
    action: Option<String>,
    #[serde(rename = "giftName")]
    gift_name: Option<String>,
    num: Option<u64>,
    combo_num: Option<u64>,
    gift_num: Option<u64>,
    combo_send: Box<Option<WsStreamCtxData>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WsStreamCtxDataMedalInfo {
    medal_name: Option<String>,
    medal_level: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WsStreamCtxDataUser {
    face: String,
    uname: String,
}

#[derive(Debug)]
pub enum WsStreamMessageType {
    DanmuMsg(DanmuMessage),
    // WELCOME_GUARD,
    // ENTRY_EFFECT,
    // WELCOME,
    // SUPER_CHAT_MESSAGE_JPN,
    SuperChatMessage(SuperChatMessage),
    InteractWord(InteractWord),
    SendGift(SendGift),
    // COMBO_SEND,
    // ANCHOR_LOT_START,
    // ANCHOR_LOT_END,
    // ANCHOR_LOT_AWARD,
    // GUARD_BUY,
    // USER_TOAST_MSG,
    // ACTIVITY_BANNER_UPDATE_V2,
    // ROOM_REAL_TIME_MESSAGE_UPDATE,
}

impl WsStreamCtx {
    pub fn new(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }

    pub fn match_msg(&self) -> Result<WsStreamMessageType> {
        let cmd = self.handle_cmd();

        let result = match cmd {
            Some("DANMU_MSG") => WsStreamMessageType::DanmuMsg(DanmuMessage::new_from_ctx(self)?),
            Some("SUPER_CHAT_MESSAGE") => {
                WsStreamMessageType::SuperChatMessage(SuperChatMessage::new_from_ctx(self)?)
            }
            Some("INTERACT_WORD") => {
                WsStreamMessageType::InteractWord(InteractWord::new_from_ctx(self)?)
            }
            Some("SEND_GIFT") | Some("COMBO_SEND") => {
                WsStreamMessageType::SendGift(SendGift::new_from_ctx(self)?)
            }
            Some(_) => return Err(anyhow!("unknown msg")),
            None => return Err(anyhow!("unknown msg")),
        };

        Ok(result)
    }

    fn handle_cmd(&self) -> Option<&str> {
        // handle DANMU_MSG:4:0:2:2:2:0
        let cmd = if let Some(c) = self.cmd.as_deref() {
            if c.starts_with("DANMU_MSG") {
                Some("DANMU_MSG")
            } else {
                Some(c)
            }
        } else {
            None
        };

        cmd
    }
}
