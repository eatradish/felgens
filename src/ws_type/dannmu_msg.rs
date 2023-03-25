use super::{util::owned, LiveMessageError, LiveMessageResult, WsStreamCtx};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DanmuMessage {
    pub uid: u64,
    pub username: String,
    pub msg: String,
    pub fan: Option<String>,
    pub fan_level: Option<u64>,
    pub timestamp: u64,
}

impl DanmuMessage {
    pub fn new_from_ctx(ctx: &WsStreamCtx) -> LiveMessageResult<Self> {
        let info = ctx
            .info
            .as_ref()
            .ok_or_else(|| LiveMessageError::DanmuMessageError(owned(ctx)))?;

        let array_2 = info
            .get(2)
            .and_then(|x| x.as_array())
            .ok_or_else(|| LiveMessageError::DanmuMessageError(owned(ctx)))?
            .to_owned();

        let uid = array_2
            .get(0)
            .and_then(|x| x.as_u64())
            .ok_or_else(|| LiveMessageError::DanmuMessageError(owned(ctx)))?;

        let username = array_2
            .get(1)
            .and_then(|x| x.as_str())
            .ok_or_else(|| LiveMessageError::DanmuMessageError(owned(ctx)))?
            .to_string();

        let msg = info
            .get(1)
            .and_then(|x| x.as_str())
            .ok_or_else(|| LiveMessageError::DanmuMessageError(owned(ctx)))?
            .to_string();

        let array_3 = info
            .get(3)
            .and_then(|x| x.as_array())
            .ok_or_else(|| LiveMessageError::DanmuMessageError(owned(ctx)))?
            .to_owned();

        let fan = array_3
            .get(1)
            .and_then(|x| x.as_str())
            .map(|x| x.to_owned());

        let fan_level = array_3.get(0).and_then(|x| x.as_u64());

        let timestamp = info
            .get(0)
            .and_then(|x| x.as_array())
            .and_then(|x| x.get(4))
            .and_then(|x| x.as_u64())
            .ok_or_else(|| LiveMessageError::DanmuMessageError(owned(ctx)))?;

        Ok(Self {
            uid,
            username,
            msg,
            fan,
            fan_level,
            timestamp,
        })
    }
}
