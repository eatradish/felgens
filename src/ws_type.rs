use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct DanmuMessage {
    pub uid: u64,
    pub username: String,
    pub msg: String,
    pub fan: Option<String>,
    pub fan_level: Option<u64>,
    pub timestamp: u64,
}

#[derive(Debug, Deserialize)]
pub struct WsStreamCtx {
    cmd: Option<String>,
    info: Option<Vec<Value>>,
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

#[derive(Debug, Deserialize)]
pub struct SuperChatMessage {
    pub uname: String,
    pub uid: u64,
    pub face: String,
    pub price: u32,
    pub start_time: u64,
    pub time: u32,
    pub msg: String,
    pub madel_name: Option<String>,
    pub madel_level: Option<u32>,
}

#[derive(Debug)]
pub struct InteractWord {
    pub uid: u64,
    pub uname: String,
    pub fan: Option<String>,
    pub fan_level: Option<u32>,
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

    fn danmu_msg(&self) -> Result<DanmuMessage> {
        let info = self
            .info
            .as_ref()
            .ok_or_else(|| anyhow!("Should have info field!"))?;

        let array_2 = info
            .get(2)
            .ok_or_else(|| trans_err("array_2", 1))?
            .as_array()
            .ok_or_else(|| trans_err("array_2", 2))?
            .to_owned();

        let uid = array_2
            .get(0)
            .ok_or_else(|| trans_err("uid", 1))?
            .as_u64()
            .ok_or_else(|| trans_err("uid", 2))?;

        let username = array_2
            .get(1)
            .ok_or_else(|| trans_err("username", 1))?
            .as_str()
            .ok_or_else(|| trans_err("username", 2))?
            .to_string();

        let msg = info
            .get(1)
            .ok_or_else(|| trans_err("msg", 1))?
            .as_str()
            .ok_or_else(|| trans_err("msg", 2))?
            .to_string();

        let array_3 = info
            .get(3)
            .ok_or_else(|| trans_err("array_3", 1))?
            .as_array()
            .ok_or_else(|| trans_err("array_3", 2))?
            .to_owned();

        let fan = array_3
            .get(1)
            .and_then(|x| x.as_str())
            .map(|x| x.to_owned());

        let fan_level = array_3.get(0).and_then(|x| x.as_u64());

        let timestamp = info
            .get(0)
            .ok_or_else(|| trans_err("timestamp", 1))?
            .as_array()
            .ok_or_else(|| trans_err("timestamp", 2))?
            .get(4)
            .ok_or_else(|| trans_err("timestamp", 3))?
            .as_u64()
            .ok_or_else(|| trans_err("trans_err", 4))?;

        Ok(DanmuMessage {
            uid,
            username,
            msg,
            fan,
            fan_level,
            timestamp,
        })
    }

    fn super_chat(&self) -> Result<SuperChatMessage> {
        let data = self
            .data
            .as_ref()
            .ok_or_else(|| anyhow!("Not a super chat message!"))?;

        let user_info = data
            .user_info
            .as_ref()
            .ok_or_else(|| anyhow!("Can not get sc user info"))?;

        let uname = user_info.uname.to_owned();

        let uid = data
            .uid
            .as_ref()
            .ok_or_else(|| anyhow!("Can not get sc uid"))?
            .as_u64()
            .ok_or_else(|| anyhow!("Can not get sc uid step 2!"))?;

        let face = user_info.face.to_owned();

        let price = data.price.ok_or_else(|| anyhow!("Can not get sc price!"))?;

        let start_time = data
            .start_time
            .ok_or_else(|| anyhow!("Can not get sc start_time!"))?;

        let time = data.time.ok_or_else(|| anyhow!("Can not get sc time!"))?;

        let msg = data
            .message
            .as_ref()
            .ok_or_else(|| anyhow!("Can not get sc message!"))?
            .to_owned();

        let madel = data
            .medal_info
            .as_ref()
            .map(|x| (x.medal_name.to_owned(), x.medal_level.to_owned()));

        let madel_name = madel.as_ref().and_then(|(name, _)| name.to_owned());

        let madel_level = madel.and_then(|(_, level)| level);

        Ok(SuperChatMessage {
            uname,
            uid,
            face,
            price,
            start_time,
            time,
            msg,
            madel_name,
            madel_level,
        })
    }

    fn interact_word(&self) -> Result<InteractWord> {
        let data = self
            .data
            .as_ref()
            .ok_or_else(|| anyhow!("Not a interact word message!"))?;

        let uname = data
            .uname
            .as_ref()
            .ok_or_else(|| anyhow!("Can not get interact uname"))?
            .to_string();

        let uid = data
            .uid
            .as_ref()
            .ok_or_else(|| anyhow!("uid doesn exist"))?
            .as_u64()
            .ok_or_else(|| anyhow!("Can not uid trans to u64"))?;

        let fan = data
            .fans_medal
            .as_ref()
            .and_then(|x| x.medal_name.to_owned());

        let fan = if fan == Some("".to_string()) {
            None
        } else {
            fan
        };

        let fan_level = data.fans_medal.as_ref().and_then(|x| x.medal_level);

        let fan_level = if fan_level == Some(0) {
            None
        } else {
            fan_level
        };

        Ok(InteractWord {
            uid,
            uname,
            fan,
            fan_level,
        })
    }
}

fn trans_err(name: &str, step: u32) -> anyhow::Error {
    anyhow!("Can not get {} step {}", name, step)
}
