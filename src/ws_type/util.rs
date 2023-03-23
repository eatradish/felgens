use super::WsStreamCtx;

pub fn owned(ctx: &WsStreamCtx) -> WsStreamCtx {
    WsStreamCtx {
        cmd: ctx.cmd.clone(),
        info: ctx.info.clone(),
        data: ctx.data.clone(),
    }
}
