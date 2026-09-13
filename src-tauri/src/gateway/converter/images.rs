//! 图片处理：data URL / 远程图片解析，以及远程 URL 的 SSRF 防护。

use super::*;

pub async fn extract_images(client: &Client, content: Option<&Value>) -> Vec<ImageBlock> {
    let Some(Value::Array(items)) = content else {
        return Vec::new();
    };

    let mut images = Vec::new();
    for item in items {
        if let Some(image) = extract_image_block(client, item).await {
            images.push(image);
        }
    }
    images
}

pub async fn extract_image_block(client: &Client, item: &Value) -> Option<ImageBlock> {
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();
    match item_type {
        "image" => {
            let source = item.get("source")?;
            let bytes = source
                .get("data")
                .and_then(Value::as_str)
                .map(str::to_string)?;
            if encoded_image_exceeds_limit(&bytes) {
                return None;
            }
            let media_type = source
                .get("media_type")
                .and_then(Value::as_str)
                .unwrap_or("image/png");
            Some(ImageBlock {
                format: media_type_to_format(media_type)?,
                source: ImageSource::Bytes { bytes },
            })
        }
        "image_url" => {
            let url = item
                .get("image_url")
                .and_then(|value| value.get("url").or(Some(value)))
                .and_then(Value::as_str)?;
            let (format, bytes) = resolve_image_source(client, url).await?;
            Some(ImageBlock {
                format,
                source: ImageSource::Bytes { bytes },
            })
        }
        "input_image" => {
            let url = item
                .get("image_url")
                .and_then(Value::as_str)
                .or_else(|| item.get("url").and_then(Value::as_str))?;
            let (format, bytes) = resolve_image_source(client, url).await?;
            Some(ImageBlock {
                format,
                source: ImageSource::Bytes { bytes },
            })
        }
        _ => None,
    }
}

pub fn media_type_to_format(media_type: &str) -> Option<String> {
    match media_type.trim().to_ascii_lowercase().as_str() {
        "image/png" | "png" => Some("png".to_string()),
        "image/jpeg" | "image/jpg" | "jpeg" | "jpg" => Some("jpeg".to_string()),
        "image/gif" | "gif" => Some("gif".to_string()),
        "image/webp" | "webp" => Some("webp".to_string()),
        _ => None,
    }
}

pub fn parse_data_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("data:")?;
    let (meta, bytes) = rest.split_once(',')?;
    let media_type = meta.split(';').next().unwrap_or_default();
    if encoded_image_exceeds_limit(bytes) {
        return None;
    }
    Some((media_type_to_format(media_type)?, bytes.to_string()))
}

pub async fn resolve_image_source(client: &Client, url: &str) -> Option<(String, String)> {
    let _ = client;
    if let Some(parsed) = parse_data_url(url) {
        return Some(parsed);
    }

    let image_client = apply_app_proxy(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(IMAGE_FETCH_TIMEOUT_SECONDS))
            .redirect(reqwest::redirect::Policy::none()),
    )
    .ok()?
    .build()
    .ok()?;
    let mut current_url = validate_remote_image_url(url).await?;

    for _ in 0..=MAX_IMAGE_REDIRECTS {
        let response = image_client.get(current_url.clone()).send().await.ok()?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)?
                .to_str()
                .ok()?;
            let next_url = current_url.join(location).ok()?;
            current_url = validate_remote_image_url(next_url.as_str()).await?;
            continue;
        }
        if !response.status().is_success() {
            return None;
        }

        if response
            .content_length()
            .map(|length| length > MAX_IMAGE_SOURCE_BYTES as u64)
            .unwrap_or(false)
        {
            return None;
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let final_url = response.url().clone();
        let bytes = response.bytes().await.ok()?;
        if bytes.len() > MAX_IMAGE_SOURCE_BYTES {
            return None;
        }
        let format = content_type
            .as_deref()
            .and_then(|value| value.split(';').next())
            .and_then(media_type_to_format)
            .or_else(|| infer_image_format_from_url(final_url.as_str()))?;

        return Some((format, STANDARD.encode(bytes)));
    }

    None
}

pub fn infer_image_format_from_url(url: &str) -> Option<String> {
    let path = reqwest::Url::parse(url).ok()?.path().to_ascii_lowercase();
    if path.ends_with(".png") {
        Some("png".to_string())
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        Some("jpeg".to_string())
    } else if path.ends_with(".gif") {
        Some("gif".to_string())
    } else if path.ends_with(".webp") {
        Some("webp".to_string())
    } else {
        None
    }
}

pub async fn validate_remote_image_url(url: &str) -> Option<reqwest::Url> {
    let parsed = reqwest::Url::parse(url).ok()?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return None,
    }

    let host = parsed.host_str()?;
    if host.eq_ignore_ascii_case("localhost") {
        return None;
    }

    let port = parsed.port_or_known_default()?;
    let mut resolved_any = false;
    for address in lookup_host((host, port)).await.ok()? {
        resolved_any = true;
        if is_restricted_remote_ip(address.ip()) {
            return None;
        }
    }

    if !resolved_any {
        return None;
    }

    Some(parsed)
}

pub fn encoded_image_exceeds_limit(encoded: &str) -> bool {
    encoded.len() > max_base64_len_for_bytes(MAX_IMAGE_SOURCE_BYTES)
}

pub fn max_base64_len_for_bytes(max_bytes: usize) -> usize {
    max_bytes.div_ceil(3) * 4
}

pub fn is_restricted_remote_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(addr) => {
            addr.is_private()
                || addr.is_loopback()
                || addr.is_link_local()
                || addr.is_broadcast()
                || addr.is_documentation()
                || addr.is_unspecified()
                || addr.is_multicast()
                || is_ipv4_shared(addr)
                || is_ipv4_reserved(addr)
        }
        IpAddr::V6(addr) => {
            addr.is_loopback()
                || addr.is_unspecified()
                || addr.is_multicast()
                || addr.is_unique_local()
                || addr.is_unicast_link_local()
                || is_ipv6_documentation(addr)
        }
    }
}

pub fn is_ipv4_shared(addr: Ipv4Addr) -> bool {
    let octets = addr.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

pub fn is_ipv4_reserved(addr: Ipv4Addr) -> bool {
    let octets = addr.octets();
    octets[0] >= 240
}

pub fn is_ipv6_documentation(addr: Ipv6Addr) -> bool {
    let segments = addr.segments();
    segments[0] == 0x2001 && segments[1] == 0x0db8
}
