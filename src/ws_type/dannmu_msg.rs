use super::{util::trans_err, WsStreamCtx};
use anyhow::{anyhow, Result};
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
    pub fn new_from_ctx(ctx: &WsStreamCtx) -> Result<Self> {
        let info = ctx
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
