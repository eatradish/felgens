use super::WsStreamCtx;
use anyhow::{anyhow, Result};

#[derive(Debug)]
pub struct InteractWord {
    pub uid: u64,
    pub uname: String,
    pub fan: Option<String>,
    pub fan_level: Option<u32>,
}

impl InteractWord {
    pub fn new_from_ctx(ctx: &WsStreamCtx) -> Result<Self> {
        let data = ctx
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

        Ok(Self {
            uid,
            uname,
            fan,
            fan_level,
        })
    }
}
