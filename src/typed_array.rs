use std::slice;

use rquickjs::{ArrayBuffer, Ctx, Exception, Object, Result, Value, function::Constructor};

pub fn value_as_u32(value: &Value<'_>) -> Option<u32> {
    let number = value.as_number()?;
    if !number.is_finite() || number < 0.0 || number > u32::MAX as f64 {
        return None;
    }
    Some(number.trunc() as u32)
}

pub fn image_data_byte_length(ctx: &Ctx<'_>, width: u32, height: u32) -> Result<u32> {
    width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| Exception::throw_range(ctx, "ImageData pixel byte length overflow"))
}

pub fn new_uint8_clamped_array<'js>(ctx: Ctx<'js>, byte_length: u32) -> Result<Object<'js>> {
    let constructor: Constructor<'js> = ctx.globals().get("Uint8ClampedArray")?;
    constructor.construct((byte_length,))
}

pub fn new_uint8_clamped_array_from_bytes<'js>(ctx: Ctx<'js>, bytes: &[u8]) -> Result<Object<'js>> {
    let array = new_uint8_clamped_array(ctx.clone(), bytes.len() as u32)?;
    with_object_bytes_mut(&ctx, &array, |target| {
        target.copy_from_slice(bytes);
        Ok(())
    })?;
    Ok(array)
}

pub fn with_value_bytes<'js, R>(
    ctx: &Ctx<'js>,
    value: Value<'js>,
    callback: impl FnOnce(&[u8]) -> Result<R>,
) -> Result<R> {
    if let Some(object) = value.as_object() {
        if let Some(buffer) = ArrayBuffer::from_object(object.clone()) {
            let bytes = buffer
                .as_bytes()
                .ok_or_else(|| Exception::throw_message(ctx, "RGBA buffer is detached"))?;
            return callback(bytes);
        }

        return with_object_bytes(ctx, object, callback);
    }

    Err(Exception::throw_type(
        ctx,
        "RGBA data must be a Uint8Array or Uint8ClampedArray",
    ))
}

pub fn with_object_bytes<'js, R>(
    ctx: &Ctx<'js>,
    object: &Object<'js>,
    callback: impl FnOnce(&[u8]) -> Result<R>,
) -> Result<R> {
    let buffer: ArrayBuffer<'js> = object.get("buffer").map_err(|_| {
        Exception::throw_type(ctx, "RGBA data must be a Uint8Array or Uint8ClampedArray")
    })?;
    let byte_offset: usize = object.get("byteOffset")?;
    let byte_length: usize = object.get("byteLength")?;
    let bytes = buffer
        .as_bytes()
        .ok_or_else(|| Exception::throw_message(ctx, "RGBA buffer is detached"))?;
    let end = byte_offset
        .checked_add(byte_length)
        .ok_or_else(|| Exception::throw_range(ctx, "RGBA byte range overflow"))?;
    let bytes = bytes
        .get(byte_offset..end)
        .ok_or_else(|| Exception::throw_range(ctx, "RGBA byte range is out of bounds"))?;
    callback(bytes)
}

pub fn with_object_bytes_mut<'js, R>(
    ctx: &Ctx<'js>,
    object: &Object<'js>,
    callback: impl FnOnce(&mut [u8]) -> Result<R>,
) -> Result<R> {
    let buffer: ArrayBuffer<'js> = object.get("buffer").map_err(|_| {
        Exception::throw_type(ctx, "RGBA data must be a Uint8Array or Uint8ClampedArray")
    })?;
    let byte_offset: usize = object.get("byteOffset")?;
    let byte_length: usize = object.get("byteLength")?;
    let bytes = buffer
        .as_bytes()
        .ok_or_else(|| Exception::throw_message(ctx, "RGBA buffer is detached"))?;
    let end = byte_offset
        .checked_add(byte_length)
        .ok_or_else(|| Exception::throw_range(ctx, "RGBA byte range overflow"))?;
    if end > bytes.len() {
        return Err(Exception::throw_range(
            ctx,
            "RGBA byte range is out of bounds",
        ));
    }

    let ptr = unsafe { (bytes.as_ptr() as *mut u8).add(byte_offset) };
    let bytes = unsafe { slice::from_raw_parts_mut(ptr, byte_length) };
    callback(bytes)
}
