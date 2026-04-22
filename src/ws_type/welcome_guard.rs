use serde::Deserialize;

use crate::{LiveMessageError, LiveMessageResult, ws_type::WsStreamCtx};

#[derive(Debug, Deserialize)]
pub struct WelcomeGuard {
    pub uid: u64,
    pub username: String,
    pub guard_level: u64, // 1: 总督, 2: 提督, 3: 舰长
}

impl WelcomeGuard {
    pub fn new_from_ctx(ctx: &'_ WsStreamCtx) -> LiveMessageResult<'_, Self> {
        let data = ctx.data.as_ref()
            .ok_or_else(|| LiveMessageError::WelcomeGuardError(ctx.clone()))?;

        Ok(Self {
            uid: data.uid.as_ref().and_then(|v| v.as_u64()).unwrap_or(0),
            username: data.uname.clone().unwrap_or_else(|| "".into()),
            guard_level: data.guard_level.unwrap_or(0),
        })
    }
}
