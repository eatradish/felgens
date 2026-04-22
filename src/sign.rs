use md5::{Digest, Md5};
use reqwest::header::HeaderMap;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::http_client::HttpClient;
use crate::FelgensError;

const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

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
pub async fn get_wbi_keys(
    client: &HttpClient,
    header: HeaderMap,
) -> Result<(String, String), FelgensError> {
    let (img_url, sub_url, _) = client.get_nav(header).await?;

    Ok((img_url, sub_url))
}

pub async fn sign_request(
    client: &HttpClient,
    params: BTreeMap<String, String>,
    header: HeaderMap,
) -> Result<String, FelgensError> {
    let (img_key, sub_key) = get_wbi_keys(client, header).await?;

    Ok(enc_wbi(params, &img_key, &sub_key))
}
