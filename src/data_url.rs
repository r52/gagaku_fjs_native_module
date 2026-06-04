use anyhow::bail;
use base64_simd::STANDARD;

pub fn decode_base64_data_url(value: &str) -> anyhow::Result<Vec<u8>> {
    let Some(comma) = value.find(',') else {
        bail!("Paperback image polyfill only supports data URLs");
    };
    if comma == 0 || !value[..comma].to_ascii_lowercase().contains(";base64") {
        bail!("Paperback image polyfill only supports data URLs");
    }

    STANDARD
        .decode_to_vec(value[comma + 1..].as_bytes())
        .map_err(|err| anyhow::anyhow!("Invalid base64 image data URL: {err}"))
}
