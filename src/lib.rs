//! Gagaku native modules for fjs.

use std::io::Write;

use anyhow::{Context as _, bail};
use base64_simd::STANDARD;
use crc32fast::Hasher;
use fjs_native_extensions::rquickjs::{
    ArrayBuffer, Ctx, Exception, Function, Object, Result, TypedArray, Value,
};
use fjs_native_extensions::{NativeExtension, NativeExtensionFactory};
use flate2::{Compression, write::ZlibEncoder};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const PNG_DATA_URL_PREFIX: &str = "data:image/png;base64,";

#[linkme::distributed_slice(fjs_native_extensions::NATIVE_EXTENSIONS)]
static GAGAKU_NATIVE_MODULE_EXTENSION: NativeExtensionFactory = extension;

/// Returns the native extension registration for Gagaku native modules.
pub fn extension() -> NativeExtension {
    NativeExtension::new().with_global(init)
}

fn init(ctx: &Ctx<'_>) -> Result<()> {
    let globals = ctx.globals();
    let gagaku = match globals.get::<_, Value<'_>>("gagaku") {
        Ok(value) => {
            if let Some(object) = value.as_object() {
                object.clone()
            } else {
                let object = Object::new(ctx.clone())?;
                globals.set("gagaku", object.clone())?;
                object
            }
        }
        Err(_) => {
            let object = Object::new(ctx.clone())?;
            globals.set("gagaku", object.clone())?;
            object
        }
    };

    let native_image = Object::new(ctx.clone())?;
    let encode_png_data_url =
        Function::new(ctx.clone(), encode_png_data_url)?.with_name("encodePngDataUrl")?;

    native_image.set("encodePngDataUrl", encode_png_data_url)?;
    gagaku.set("nativeImage", native_image)?;

    Ok(())
}

fn encode_png_data_url<'js>(
    ctx: Ctx<'js>,
    rgba: Value<'js>,
    width: u32,
    height: u32,
) -> Result<String> {
    if let Ok(typed_array) = TypedArray::<u8>::from_value(rgba.clone()) {
        let rgba = typed_array
            .as_bytes()
            .ok_or_else(|| Exception::throw_message(&ctx, "RGBA buffer is detached"))?;
        return encode_png_data_url_from_rgba(rgba, width, height)
            .map_err(|err| Exception::throw_message(&ctx, &err.to_string()));
    }

    if let Some(object) = rgba.as_object() {
        if let Some(buffer) = ArrayBuffer::from_object(object.clone()) {
            let rgba = buffer
                .as_bytes()
                .ok_or_else(|| Exception::throw_message(&ctx, "RGBA buffer is detached"))?;
            return encode_png_data_url_from_rgba(rgba, width, height)
                .map_err(|err| Exception::throw_message(&ctx, &err.to_string()));
        }

        let buffer: ArrayBuffer<'js> = object.get("buffer").map_err(|_| {
            Exception::throw_type(&ctx, "RGBA data must be a Uint8Array or Uint8ClampedArray")
        })?;
        let byte_offset: usize = object.get("byteOffset")?;
        let byte_length: usize = object.get("byteLength")?;
        let bytes = buffer
            .as_bytes()
            .ok_or_else(|| Exception::throw_message(&ctx, "RGBA buffer is detached"))?;
        let end = byte_offset
            .checked_add(byte_length)
            .ok_or_else(|| Exception::throw_range(&ctx, "RGBA byte range overflow"))?;
        let rgba = bytes
            .get(byte_offset..end)
            .ok_or_else(|| Exception::throw_range(&ctx, "RGBA byte range is out of bounds"))?;

        return encode_png_data_url_from_rgba(rgba, width, height)
            .map_err(|err| Exception::throw_message(&ctx, &err.to_string()));
    }

    Err(Exception::throw_type(
        &ctx,
        "RGBA data must be a Uint8Array or Uint8ClampedArray",
    ))
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_rgba_to_png_data_url() {
        let data_url = encode_png_data_url_from_rgba(&[255, 0, 0, 255], 1, 1).unwrap();

        assert!(data_url.starts_with(PNG_DATA_URL_PREFIX));
        assert!(data_url.len() > PNG_DATA_URL_PREFIX.len());
    }

    #[test]
    fn rejects_rgba_length_mismatch() {
        let err = encode_png_data_url_from_rgba(&[255, 0, 0], 1, 1).unwrap_err();

        assert!(err.to_string().contains("RGBA byte length mismatch"));
    }
}
