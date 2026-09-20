use reqwest::{header::HeaderMap, Client, Response};
use serde::Deserialize;
use std::{collections::BTreeMap, time::Duration};
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
}
