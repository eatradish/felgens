use serde::Deserialize;

use super::{LiveMessageError, LiveMessageResult, WsStreamCtx};

#[derive(Debug, Deserialize)]
pub struct SendGift {
    pub action: String,
    pub gift_name: String,
    pub num: u64,
    pub uname: String,
    pub uid: u64,
    pub medal_name: Option<String>,
    pub medal_level: Option<u32>,
    pub price: u32,
}

impl SendGift {
    pub fn new_from_ctx(ctx: &WsStreamCtx) -> LiveMessageResult<Self> {
        let data = ctx
            .data
            .as_ref()
            .ok_or_else(|| LiveMessageError::SendGiftMessageError(ctx.clone()))?;

        let action = data
            .action
            .as_ref()
            .ok_or_else(|| LiveMessageError::SendGiftMessageError(ctx.clone()))?
            .to_owned();

        let combo_send = data.combo_send.clone();

        let gift_name = if let Some(gift) = data.gift_name.as_ref() {
            gift.to_owned()
        } else if let Some(gift) = combo_send.clone().and_then(|x| x.gift_name) {
            gift
        } else {
            return Err(LiveMessageError::SendGiftMessageError(ctx.clone()));
        };

        let num = if let Some(num) = combo_send.clone().and_then(|x| x.combo_num) {
            num
        } else if let Some(num) = data.num {
            num
        } else if let Some(num) = combo_send.and_then(|x| x.gift_num) {
            num
        } else {
            return Err(LiveMessageError::SendGiftMessageError(ctx.clone()));
        };

        let uname = data
            .uname
            .as_ref()
            .ok_or_else(|| LiveMessageError::SendGiftMessageError(ctx.clone()))?
            .to_owned();

        let uid = data
            .uid
            .as_ref()
            .ok_or_else(|| LiveMessageError::SendGiftMessageError(ctx.clone()))?
            .as_u64()
            .ok_or_else(|| LiveMessageError::SendGiftMessageError(ctx.clone()))?;

        let medal_name = data
            .medal_info
            .as_ref()
            .and_then(|x| x.medal_name.to_owned());

        let medal_level = data.medal_info.as_ref().and_then(|x| x.medal_level);

        let medal_name = if medal_name == Some("".to_string()) {
            None
        } else {
            medal_name
        };

        let medal_level = if medal_level == Some(0) {
            None
        } else {
            medal_level
        };

        let price = data
            .price
            .ok_or_else(|| LiveMessageError::SendGiftMessageError(ctx.clone()))?;

        Ok(Self {
            action,
            gift_name,
            num,
            uname,
            uid,
            medal_name,
            medal_level,
            price,
        })
    }
}
