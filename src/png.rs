use std::io::Write;

use anyhow::{Context as _, bail};
use base64_simd::STANDARD;
use crc32fast::Hasher;
use flate2::{Compression, write::ZlibEncoder};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
pub const PNG_DATA_URL_PREFIX: &str = "data:image/png;base64,";

/// Encodes RGBA pixels into a PNG data URL.
pub fn encode_png_data_url_from_rgba(
    rgba: &[u8],
    width: u32,
    height: u32,
) -> anyhow::Result<String> {
    let png = encode_png_from_rgba(rgba, width, height)?;
    let mut data_url =
        String::with_capacity(PNG_DATA_URL_PREFIX.len() + STANDARD.encoded_length(png.len()));

    data_url.push_str(PNG_DATA_URL_PREFIX);
    STANDARD.encode_append(png, &mut data_url);

    Ok(data_url)
}

fn encode_png_from_rgba(rgba: &[u8], width: u32, height: u32) -> anyhow::Result<Vec<u8>> {
    if width == 0 || height == 0 {
        bail!("PNG width and height must be greater than zero");
    }

    let width = width as usize;
    let height = height as usize;
    let row_len = width
        .checked_mul(4)
        .context("PNG row byte length overflow")?;
    let expected_len = row_len
        .checked_mul(height)
        .context("PNG RGBA byte length overflow")?;

    if rgba.len() != expected_len {
        bail!(
            "RGBA byte length mismatch: expected {}, got {}",
            expected_len,
            rgba.len()
        );
    }

    let mut zlib = ZlibEncoder::new(
        Vec::with_capacity(expected_len + height),
        Compression::none(),
    );
    for row in rgba.chunks_exact(row_len) {
        zlib.write_all(&[0])?;
        zlib.write_all(row)?;
    }
    let idat = zlib.finish()?;

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);

    let mut png = Vec::with_capacity(PNG_SIGNATURE.len() + 12 + ihdr.len() + 12 + idat.len() + 12);
    png.extend_from_slice(PNG_SIGNATURE);
    write_chunk(&mut png, b"IHDR", &ihdr)?;
    write_chunk(&mut png, b"IDAT", &idat)?;
    write_chunk(&mut png, b"IEND", &[])?;

    Ok(png)
}

fn write_chunk(png: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) -> anyhow::Result<()> {
    let data_len = u32::try_from(data.len()).context("PNG chunk is too large")?;
    png.extend_from_slice(&data_len.to_be_bytes());
    png.extend_from_slice(chunk_type);
    png.extend_from_slice(data);

    let mut hasher = Hasher::new();
    hasher.update(chunk_type);
    hasher.update(data);
    png.extend_from_slice(&hasher.finalize().to_be_bytes());

    Ok(())
}
