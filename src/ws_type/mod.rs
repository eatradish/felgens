mod dannmu_msg;
mod super_chat;
mod interact_word;

use serde::Deserialize;
use serde_json::Value;
use anyhow::{Result, anyhow};

pub use self::dannmu_msg::DanmuMessage;
pub use self::super_chat::SuperChatMessage;
pub use self::interact_word::InteractWord;

#[derive(Debug, Deserialize)]
pub struct WsStreamCtx {
    cmd: Option<String>,
    info: Option<Vec<serde_json::Value>>,
    data: Option<WsStreamCtxData>,
}

#[derive(Debug, Deserialize)]
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
}

#[derive(Debug, Deserialize)]
pub struct WsStreamCtxDataMedalInfo {
    medal_name: Option<String>,
    medal_level: Option<u32>,
}

#[derive(Debug, Deserialize)]
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
    // SEND_GIFT,
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
        let result = match self.cmd.as_deref() {
            Some("DANMU_MSG") => WsStreamMessageType::DanmuMsg(self.danmu_msg()?),
            Some("SUPER_CHAT_MESSAGE") => WsStreamMessageType::SuperChatMessage(self.super_chat()?),
            Some("INTERACT_WORD") => WsStreamMessageType::InteractWord(self.interact_word()?),
            Some(_) => return Err(anyhow!("unknown msg")),
            None => return Err(anyhow!("unknown msg")),
        };

        Ok(result)
    }

    pub fn danmu_msg(&self) -> Result<DanmuMessage> {
        DanmuMessage::new_from_ctx(self)
    }

    pub fn super_chat(&self) -> Result<SuperChatMessage> {
        SuperChatMessage::new_from_ctx(self)
    }

    pub fn interact_word(&self) -> Result<InteractWord> {
        InteractWord::new_from_ctx(self)
    }
}