use reqwest::{header::HeaderMap, Client, Response};
use serde::Deserialize;
use std::{collections::BTreeMap, time::Duration};
use url::Url;

use crate::{sign::sign_request, FelgensResult};

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
pub struct RoomInit {
    data: RoomInitData,
}

#[derive(Debug, Deserialize)]
pub struct RoomInitData {
    room_id: u64,
}

#[derive(Debug, Deserialize)]
struct NavResponse {
    data: NavData,
}

#[derive(Debug, Deserialize)]
struct NavData {
    mid: u64,
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
        headers: Option<HeaderMap>,
    ) -> FelgensResult<DanmuInfo> {
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), room_id.to_string());
        params.insert("type".to_string(), "0".to_string());
        params.insert("web_location".to_string(), "444.8".to_string());

        let sign = sign_request(params).await?;

        let resp = self
            .get_live(
                &format!("xlive/web-room/v1/index/getDanmuInfo?{}", sign),
                None,
                headers,
            )
            .await?
            .json::<DanmuInfo>()
            .await?;

        Ok(resp)
    }

    pub async fn get_uid(&self, headers: HeaderMap) -> FelgensResult<u64> {
        let resp = self
            .get("x/web-interface/nav", None, Some(headers))
            .await?
            .json::<NavResponse>()
            .await?;

        Ok(resp.data.mid)
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
            .await?
            .json::<RoomInit>()
            .await?
            .data
            .room_id;

        Ok(resp)
    }
}
