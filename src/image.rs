use std::time::Instant;

use image as image_rs;
use rquickjs::{Ctx, IntoJs, Object, Result, Value};

use crate::data_url::decode_base64_data_url;
use crate::logging::{log_image_error, profile_image_operation};
use crate::typed_array::new_uint8_clamped_array_from_bytes;

#[derive(rquickjs::class::Trace, rquickjs::JsLifetime)]
#[rquickjs::class(rename = "Image")]
pub struct NativeImage<'js> {
    onload: Value<'js>,
    onerror: Value<'js>,
    complete: bool,
    natural_width: u32,
    natural_height: u32,
    width: u32,
    height: u32,
    pixels: Object<'js>,
    src: String,
}

#[rquickjs::methods]
impl<'js> NativeImage<'js> {
    #[qjs(constructor)]
    pub fn new(ctx: Ctx<'js>) -> Result<Self> {
        Ok(Self {
            onload: Value::new_null(ctx.clone()),
            onerror: Value::new_null(ctx.clone()),
            complete: false,
            natural_width: 0,
            natural_height: 0,
            width: 0,
            height: 0,
            pixels: new_uint8_clamped_array_from_bytes(ctx, &[])?,
            src: String::new(),
        })
    }

    #[qjs(get)]
    pub fn onload(&self) -> Value<'js> {
        self.onload.clone()
    }

    #[qjs(set, rename = "onload")]
    pub fn set_onload(&mut self, value: Value<'js>) {
        self.onload = value;
    }

    #[qjs(get)]
    pub fn onerror(&self) -> Value<'js> {
        self.onerror.clone()
    }

    #[qjs(set, rename = "onerror")]
    pub fn set_onerror(&mut self, value: Value<'js>) {
        self.onerror = value;
    }

    #[qjs(get)]
    pub fn complete(&self) -> bool {
        self.complete
    }

    #[qjs(get, rename = "naturalWidth")]
    pub fn natural_width(&self) -> u32 {
        self.natural_width
    }

    #[qjs(get, rename = "naturalHeight")]
    pub fn natural_height(&self) -> u32 {
        self.natural_height
    }

    #[qjs(get)]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[qjs(set, rename = "width")]
    pub fn set_width(&mut self, value: u32) {
        self.width = value;
    }

    #[qjs(get)]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[qjs(set, rename = "height")]
    pub fn set_height(&mut self, value: u32) {
        self.height = value;
    }

    #[qjs(get)]
    pub fn pixels(&self) -> Object<'js> {
        self.pixels.clone()
    }

    #[qjs(set, rename = "pixels")]
    pub fn set_pixels(&mut self, value: Object<'js>) {
        self.pixels = value;
    }

    #[qjs(get)]
    pub fn src(&self) -> &str {
        &self.src
    }

    #[qjs(set, rename = "src")]
    pub fn set_src(&mut self, ctx: Ctx<'js>, value: String) -> Result<()> {
        let started_at = Instant::now();
        self.src = value.clone();
        self.complete = false;

        match decode_image(&ctx, &value) {
            Ok(decoded) => {
                self.natural_width = decoded.width;
                self.natural_height = decoded.height;
                self.width = decoded.width;
                self.height = decoded.height;
                self.pixels = decoded.pixels;
                self.complete = true;
                profile_image_operation(
                    &ctx,
                    "Image.src decode",
                    started_at,
                    format!(
                        "{}x{} {} input bytes {} rgba bytes",
                        decoded.width, decoded.height, decoded.input_len, decoded.rgba_len
                    ),
                );
                call_handler(&ctx, &self.onload, create_event(&ctx, "load")?)?;
            }
            Err(err) => {
                log_image_error(
                    &ctx,
                    "Image.src decode",
                    started_at,
                    format!("{} input chars {err}", value.len()),
                );
                call_handler(&ctx, &self.onerror, err.to_string().into_js(&ctx)?)?;
            }
        }

        Ok(())
    }
}

impl<'js> NativeImage<'js> {
    pub fn complete_value(&self) -> bool {
        self.complete
    }

    pub fn natural_width_value(&self) -> u32 {
        self.natural_width
    }

    pub fn natural_height_value(&self) -> u32 {
        self.natural_height
    }

    pub fn pixels_object(&self) -> Object<'js> {
        self.pixels.clone()
    }
}

struct DecodedImage<'js> {
    width: u32,
    height: u32,
    input_len: usize,
    rgba_len: usize,
    pixels: Object<'js>,
}

fn decode_image<'js>(ctx: &Ctx<'js>, data_url: &str) -> anyhow::Result<DecodedImage<'js>> {
    let bytes = decode_base64_data_url(data_url)?;
    let input_len = bytes.len();
    let decoded = image_rs::load_from_memory(&bytes)?;
    let rgba = decoded.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let rgba_len = rgba.as_raw().len();
    let pixels = new_uint8_clamped_array_from_bytes(ctx.clone(), rgba.as_raw())
        .map_err(|err| anyhow::anyhow!("{err}"))?;

    Ok(DecodedImage {
        width,
        height,
        input_len,
        rgba_len,
        pixels,
    })
}

fn create_event<'js>(ctx: &Ctx<'js>, event_type: &str) -> Result<Value<'js>> {
    let event = Object::new(ctx.clone())?;
    event.set("type", event_type)?;
    Ok(event.into_value())
}

fn call_handler<'js>(ctx: &Ctx<'js>, handler: &Value<'js>, event: Value<'js>) -> Result<()> {
    if handler.is_null() || handler.is_undefined() {
        return Ok(());
    }

    let Some(function) = handler.as_function() else {
        return Ok(());
    };
    let _: Value<'js> = function.call((event,))?;
    let _ = ctx;
    Ok(())
}
