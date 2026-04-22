use md5::{Digest, Md5};
use reqwest::header::{REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::FelgensError;

const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

#[derive(Deserialize)]
struct NavResponse {
    data: NavData,
}

#[derive(Deserialize)]
struct NavData {
    wbi_img: WbiImg,
}

#[derive(Deserialize)]
struct WbiImg {
    img_url: String,
    sub_url: String,
}

/// 字符顺序打乱编码
fn get_mixin_key(orig: &str) -> String {
    let mut mixin_key = String::with_capacity(32);
    for &idx in MIXIN_KEY_ENC_TAB.iter() {
        if let Some(c) = orig.chars().nth(idx) {
            mixin_key.push(c);
        }
    }
    mixin_key.truncate(32);
    mixin_key
}

/// 为请求参数进行 wbi 签名
pub fn enc_wbi(mut params: BTreeMap<String, String>, img_key: &str, sub_key: &str) -> String {
    let mixin_key = get_mixin_key(&format!("{}{}", img_key, sub_key));
    let curr_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // 添加时间戳
    params.insert("wts".to_string(), curr_time.to_string());

    // 过滤并拼装 query string
    // BTreeMap 会自动按 key 排序，省去了 JS 中的 .sort()
    let filtered_query = params
        .iter()
        .map(|(k, v)| {
            // 过滤 JS 中的 / [!'()*]/g 字符
            let val = v
                .chars()
                .filter(|c| !"'()*!".contains(*c))
                .collect::<String>();
            format!("{}={}", urlencoding::encode(k), urlencoding::encode(&val))
        })
        .collect::<Vec<_>>()
        .join("&");

    // 计算 MD5 签名 (w_rid)
    let mut hasher = Md5::new();
    hasher.update(format!("{}{}", filtered_query, mixin_key).as_bytes());
    let w_rid = format!("{:x}", hasher.finalize());

    format!("{}&w_rid={}", filtered_query, w_rid)
}

/// 获取最新的 img_key 和 sub_key
pub async fn get_wbi_keys() -> Result<(String, String), FelgensError> {
    let client = reqwest::Client::new();
    let res = client
        .get("https://api.bilibili.com/x/web-interface/nav")
        .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3")
        .header(REFERER, "https://www.bilibili.com/")
        .send()
        .await?
        .json::<NavResponse>()
        .await?;

    let extract_key = |url: &str| {
        url.split('/')
            .last()
            .and_then(|s| s.split('.').next())
            .unwrap_or("")
            .to_string()
    };

    Ok((
        extract_key(&res.data.wbi_img.img_url),
        extract_key(&res.data.wbi_img.sub_url),
    ))
}

pub async fn sign_request(
    params: BTreeMap<String, String>,
) -> Result<String, FelgensError> {
    let (img_key, sub_key) = get_wbi_keys().await?;

    Ok(enc_wbi(params, &img_key, &sub_key))
}
