use serde::Deserialize;

use super::{util::owned, LiveMessageError, LiveMessageResult, WsStreamCtx};

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

impl SuperChatMessage {
    pub fn new_from_ctx(ctx: &WsStreamCtx) -> LiveMessageResult<Self> {
        let data = ctx
            .data
            .as_ref()
            .ok_or_else(|| LiveMessageError::SuperChatMessageError(owned(ctx)))?;

        let user_info = data
            .user_info
            .as_ref()
            .ok_or_else(|| LiveMessageError::SuperChatMessageError(owned(ctx)))?;

        let uname = user_info.uname.to_owned();

        let uid = data
            .uid
            .as_ref()
            .and_then(|x| x.as_u64())
            .ok_or_else(|| LiveMessageError::SuperChatMessageError(owned(ctx)))?;

        let face = user_info.face.to_owned();

        let price = data
            .price
            .ok_or_else(|| LiveMessageError::SuperChatMessageError(owned(ctx)))?;

        let start_time = data
            .start_time
            .ok_or_else(|| LiveMessageError::SuperChatMessageError(owned(ctx)))?;

        let time = data
            .time
            .ok_or_else(|| LiveMessageError::SuperChatMessageError(owned(ctx)))?;

        let msg = data
            .message
            .as_ref()
            .ok_or_else(|| LiveMessageError::SuperChatMessageError(owned(ctx)))?
            .to_owned();

        let madel = data
            .medal_info
            .as_ref()
            .map(|x| (x.medal_name.to_owned(), x.medal_level.to_owned()));

        let madel_name = madel.as_ref().and_then(|(name, _)| name.to_owned());

        let madel_level = madel.and_then(|(_, level)| level);

        Ok(Self {
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
}
