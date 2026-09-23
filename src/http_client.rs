use reqwest::{header::HeaderMap, Client, Response};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use url::Url;

use crate::{sign::sign_request, FelgensError, FelgensResult};

pub struct HttpClient {
    client: Client,
    api_live_base_url: Url,
    api_base_url: Url,
}

#[derive(Debug, Deserialize)]
pub struct DanmuInfo {
    pub data: DanmuInfoData,
}

#[derive(Debug, Deserialize)]
pub struct DanmuInfoData {
    pub token: String,
    pub host_list: Vec<WsHost>,
}

#[derive(Debug, Deserialize)]
pub struct WsHost {
    pub host: String,
}

#[derive(Debug, Deserialize)]
pub struct RoomInitData {
    room_id: u64,
}

#[derive(Debug, Deserialize)]
struct NavData {
    mid: u64,
    wbi_img: WbiImg,
}

#[derive(Debug, Deserialize)]
struct WbiImg {
    img_url: String,
    sub_url: String,
}

/// 接口应答的通用外壳：先看 `code` 再看 `data`。
///
/// 风控（-352 之类）错误的响应里没有 `data`，直接按具体结构体解析只会得到
/// 「missing field `data`」这种看不懂的报错；先过这一层就能报出 code 和人话。
#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    code: i64,
    #[serde(alias = "msg")]
    message: Option<String>,
    data: Option<T>,
}

impl<T> ApiEnvelope<T> {
    /// 把 `data` 拆出来；没有 `data` 就报出带 `code` 的人话错误。
    ///
    /// `code != 0` 但带着 `data` 的应答（比如未登录时 `nav` 的 -101）照旧放行——
    /// 这一版只把「看不懂的报错」换成人话，不改任何原来能跑通的情况。
    fn into_data(self, what: &'static str) -> FelgensResult<T> {
        match self.data {
            Some(data) => Ok(data),
            None => {
                let message = self
                    .message
                    .filter(|text| !text.is_empty())
                    .unwrap_or_else(|| "响应里没有 data".to_string());
                Err(FelgensError::ApiError {
                    what: what.to_string(),
                    code: self.code,
                    message,
                })
            }
        }
    }
}

/// 解一份接口应答：错误包会被拦成带 code 的人话错误。
async fn decode_api<T: serde::de::DeserializeOwned>(
    resp: Response,
    what: &'static str,
) -> FelgensResult<T> {
    resp.json::<ApiEnvelope<T>>().await?.into_data(what)
}

/// Reply of the `msg/send` endpoint: `code == 0` means the danmaku was sent.
///
/// A successful reply carries both `message` and `msg` (empty strings), so they
/// are read as separate fields. Aliasing one onto the other makes serde fail
/// with a "duplicate field" error.
#[derive(Debug, Deserialize)]
struct SendDanmakuReply {
    code: i64,
    message: Option<String>,
    msg: Option<String>,
}

/// Read a key from a cookie string like `a=1; b=2`.
fn cookie_value<'a>(cookie: &'a str, key: &str) -> Option<&'a str> {
    cookie.split(';').find_map(|part| {
        let (name, value) = part.split_once('=')?;
        (name.trim() == key).then(|| value.trim())
    })
}

impl HttpClient {
    pub fn new() -> FelgensResult<Self> {
        Ok(Self {
            client: Client::new(),
            api_live_base_url: Url::parse("https://api.live.bilibili.com")?,
            api_base_url: Url::parse("https://api.bilibili.com")?,
        })
    }

    async fn get_live(
        &self,
        path: &str,
        query: Option<&[(&str, &str)]>,
        headers: Option<HeaderMap>,
    ) -> FelgensResult<Response> {
        let resp = self
            .client
            .get(self.api_live_base_url.join(path)?)
            .query(query.unwrap_or_default())
            .headers(headers.unwrap_or_default())
            .timeout(Duration::from_secs(30))
            .send()
            .await?
            .error_for_status()?;

        Ok(resp)
    }

    async fn get(
        &self,
        path: &str,
        query: Option<&[(&str, &str)]>,
        headers: Option<HeaderMap>,
    ) -> FelgensResult<Response> {
        let resp = self
            .client
            .get(self.api_base_url.join(path)?)
            .query(query.unwrap_or_default())
            .headers(headers.unwrap_or_default())
            .timeout(Duration::from_secs(30))
            .send()
            .await?
            .error_for_status()?;

        Ok(resp)
    }

    pub async fn get_dammu_info(
        &self,
        room_id: u64,
        headers: HeaderMap,
    ) -> FelgensResult<DanmuInfo> {
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), room_id.to_string());
        params.insert("type".to_string(), "0".to_string());
        params.insert("web_location".to_string(), "444.8".to_string());

        let sign = sign_request(self, params, headers.clone()).await?;

        let resp = self
            .get_live(
                &format!("xlive/web-room/v1/index/getDanmuInfo?{}", sign),
                None,
                Some(headers),
            )
            .await?;
        let data = decode_api::<DanmuInfoData>(resp, "getDanmuInfo").await?;

        Ok(DanmuInfo { data })
    }

    pub async fn get_nav(&self, headers: HeaderMap) -> FelgensResult<(String, String, u64)> {
        let resp = self.get("x/web-interface/nav", None, Some(headers)).await?;
        let data = decode_api::<NavData>(resp, "nav").await?;

        let extract_key = |url: &str| {
            url.split('/')
                .next_back()
                .and_then(|s| s.split('.').next())
                .unwrap_or("")
                .to_string()
        };

        Ok((
            extract_key(&data.wbi_img.img_url),
            extract_key(&data.wbi_img.sub_url),
            data.mid,
        ))
    }

    /// Send a danmaku (live chat message) over `msg/send`.
    ///
    /// The cookie must contain `bili_jct`, which is sent as both `csrf` and
    /// `csrf_token`. The endpoint reports the result in `code`: anything non-zero
    /// (`-101` not logged in, `-111` bad csrf, `1003212` too long, `10031` too
    /// fast, ...) becomes a [`FelgensError::ApiError`].
    pub async fn send_danmaku(&self, room_id: u64, msg: &str, cookie: &str) -> FelgensResult<()> {
        let csrf = cookie_value(cookie, "bili_jct").ok_or(FelgensError::MissingCsrf)?;
        let rnd = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let form = [
            ("roomid", room_id.to_string()),
            ("msg", msg.to_string()),
            ("rnd", rnd.to_string()),
            ("fontsize", "25".to_string()),
            ("color", "16777215".to_string()),
            ("mode", "1".to_string()),
            ("bubble", "0".to_string()),
            ("csrf", csrf.to_string()),
            ("csrf_token", csrf.to_string()),
        ];

        let resp = self
            .client
            .post(self.api_live_base_url.join("msg/send")?)
            .header(reqwest::header::COOKIE, cookie)
            .form(&form)
            .timeout(Duration::from_secs(30))
            .send()
            .await?
            .error_for_status()?;

        let reply: SendDanmakuReply = resp.json().await?;
        if reply.code == 0 {
            return Ok(());
        }

        Err(FelgensError::ApiError {
            what: "msg/send".to_string(),
            code: reply.code,
            message: [reply.message, reply.msg]
                .into_iter()
                .flatten()
                .find(|text| !text.is_empty())
                .unwrap_or_else(|| "failed to send danmaku".to_string()),
        })
    }

    pub async fn get_room_id(&self, room_id: u64) -> FelgensResult<u64> {
        if room_id > 1000 {
            return Ok(room_id);
        }

        let resp = self
            .get_live(
                &format!("room/v1/Room/room_init?id={}?&from=room", room_id),
                None,
                None,
            )
            .await?;
        let data = decode_api::<RoomInitData>(resp, "room_init").await?;

        Ok(data.room_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_reports_code_and_message() {
        // 风控错误包：以前会被报成 missing field `data`
        let raw = r#"{"code":-352,"message":"风控校验失败","ttl":1}"#;
        let envelope: ApiEnvelope<NavData> = serde_json::from_str(raw).unwrap();
        let err = envelope.into_data("nav").unwrap_err();
        let text = err.to_string();
        assert!(text.contains("nav"));
        assert!(text.contains("-352"));
        assert!(text.contains("风控校验失败"));
    }

    #[test]
    fn api_error_tolerates_msg_field() {
        let raw = r#"{"code":-101,"msg":"账号未登录"}"#;
        let envelope: ApiEnvelope<RoomInitData> = serde_json::from_str(raw).unwrap();
        let err = envelope.into_data("room_init").unwrap_err();
        assert!(err.to_string().contains("账号未登录"));
    }

    #[test]
    fn api_ok_takes_data() {
        let raw = r#"{"code":0,"data":{"mid":42,"wbi_img":{"img_url":"https://x/img123.png","sub_url":"https://x/sub456.png"}}}"#;
        let envelope: ApiEnvelope<NavData> = serde_json::from_str(raw).unwrap();
        let data = envelope.into_data("nav").unwrap();
        assert_eq!(data.mid, 42);
    }

    #[test]
    fn api_error_with_data_still_proceeds() {
        // 未登录时 nav 会给 code=-101 但带着 WBI 钥匙：老样子照用，别拦
        let raw = r#"{"code":-101,"message":"账号未登录","data":{"mid":0,"wbi_img":{"img_url":"https://x/img123.png","sub_url":"https://x/sub456.png"}}}"#;
        let envelope: ApiEnvelope<NavData> = serde_json::from_str(raw).unwrap();
        let data = envelope.into_data("nav").unwrap();
        assert_eq!(data.mid, 0);
    }

    #[test]
    fn api_code_zero_without_data_is_still_an_error() {
        let raw = r#"{"code":0}"#;
        let envelope: ApiEnvelope<RoomInitData> = serde_json::from_str(raw).unwrap();
        let err = envelope.into_data("room_init").unwrap_err();
        assert!(err.to_string().contains("没有 data"));
    }

    #[test]
    fn send_reply_accepts_message_and_msg_together() {
        // The success reply carries both `message` and `msg`; aliasing them onto
        // one field would fail with a "duplicate field" error.
        let raw = r#"{"code":0,"message":"","msg":"","data":{}}"#;
        let reply: SendDanmakuReply = serde_json::from_str(raw).unwrap();
        assert_eq!(reply.code, 0);
    }

    #[test]
    fn cookie_value_is_extracted() {
        let cookie = "SESSDATA=abc%2Cdef; bili_jct=deadbeef; DedeUserID=42";
        assert_eq!(cookie_value(cookie, "bili_jct"), Some("deadbeef"));
        assert_eq!(cookie_value(cookie, "DedeUserID"), Some("42"));
        assert_eq!(cookie_value(cookie, "nope"), None);
        // Names must match exactly: bili_jct_x is not bili_jct.
        assert_eq!(cookie_value("bili_jct_x=1", "bili_jct"), None);
    }
}
