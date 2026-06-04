//! Gagaku native modules for fjs.

mod canvas;
mod context2d;
mod data_url;
mod image;
mod image_data;
mod logging;
mod png;
mod typed_array;

use canvas::NativeCanvas;
use context2d::NativeCanvasRenderingContext2D;
use fjs_native_extensions::{NativeExtension, NativeExtensionFactory};
use image::NativeImage;
use image_data::{NativeImageData, inspect_image_data};
use logging::{log_image_error, profile_image_operation};
use rquickjs::{Class, Ctx, Exception, Function, Object, Result, Value};
use std::time::Instant;
use typed_array::with_value_bytes;

pub use png::encode_png_data_url_from_rgba;

#[linkme::distributed_slice(fjs_native_extensions::NATIVE_EXTENSIONS)]
static GAGAKU_NATIVE_MODULE_EXTENSION: NativeExtensionFactory = extension;

/// Returns the native extension registration for Gagaku native modules.
pub fn extension() -> NativeExtension {
    NativeExtension::new().with_global(init)
}

fn init(ctx: &Ctx<'_>) -> Result<()> {
    let globals = ctx.globals();
    Class::<NativeImageData>::define(&globals)?;
    Class::<NativeImage>::define(&globals)?;
    Class::<NativeCanvas>::define(&globals)?;
    Class::<NativeCanvasRenderingContext2D>::define(&globals)?;

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
    let native_canvas = Object::new(ctx.clone())?;
    let inspect_image_data =
        Function::new(ctx.clone(), inspect_image_data)?.with_name("inspectImageData")?;

    native_image.set("encodePngDataUrl", encode_png_data_url)?;
    native_canvas.set("inspectImageData", inspect_image_data)?;
    gagaku.set("nativeImage", native_image)?;
    gagaku.set("nativeCanvas", native_canvas)?;

    Ok(())
}

fn encode_png_data_url<'js>(
    ctx: Ctx<'js>,
    rgba: Value<'js>,
    width: u32,
    height: u32,
) -> Result<String> {
    let started_at = Instant::now();
    let mut rgba_len = 0;
    let result = with_value_bytes(&ctx, rgba, |rgba| {
        rgba_len = rgba.len();
        encode_png_data_url_from_rgba(rgba, width, height)
            .map_err(|err| Exception::throw_message(&ctx, &err.to_string()))
    });

    match &result {
        Ok(data_url) => profile_image_operation(
            &ctx,
            "nativeImage.encodePngDataUrl",
            started_at,
            format!(
                "{width}x{height} {rgba_len} rgba bytes {} chars",
                data_url.len()
            ),
        ),
        Err(err) => log_image_error(
            &ctx,
            "nativeImage.encodePngDataUrl",
            started_at,
            format!("{width}x{height} {rgba_len} rgba bytes {err}"),
        ),
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use png::PNG_DATA_URL_PREFIX;
    use rquickjs::{Context, Runtime};

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

    #[test]
    fn native_image_data_allocates_uint8_clamped_array() {
        with_context(|ctx| {
            let result: String = ctx
                .eval(
                    r#"
                    JSON.stringify((() => {
                      const imageData = new ImageData(2, 3);
                      return [
                        imageData.width,
                        imageData.height,
                        imageData.data.length,
                        imageData.colorSpace,
                        imageData.data instanceof Uint8ClampedArray
                      ];
                    })())
                    "#,
                )
                .unwrap();

            assert_eq!(result, r#"[2,3,24,"srgb",true]"#);
        });
    }

    #[test]
    fn native_image_data_exposes_js_mutations_to_rust() {
        with_context(|ctx| {
            let result: String = ctx
                .eval(
                    r#"
                    JSON.stringify((() => {
                      const imageData = new ImageData(new Uint8ClampedArray(8), 1);
                      imageData.data[0] = 5;
                      imageData.data[1] = 260;
                      imageData.data[2] = -20;
                      imageData.data[3] = 7;
                      const probe = gagaku.nativeCanvas.inspectImageData(imageData);
                      return [
                        probe.width,
                        probe.height,
                        probe.length,
                        probe.checksum,
                        probe.first,
                        probe.second,
                        probe.third,
                        probe.fourth
                      ];
                    })())
                    "#,
                )
                .unwrap();

            assert_eq!(result, "[1,2,8,267,5,255,0,7]");
        });
    }

    #[test]
    fn native_canvas_draws_and_round_trips_png_data_url() {
        with_context(|ctx| {
            let result: String = ctx
                .eval(
                    r#"
                    JSON.stringify((() => {
                      const image = new Image();
                      let loaded = false;
                      image.onload = () => { loaded = true; };
                      image.src = "data:image/bmp;base64,Qk06AAAAAAAAADYAAAAoAAAAAQAAAAEAAAABABgAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAD/AA==";
                      const canvas = new HTMLCanvasElement();
                      canvas.width = 1;
                      canvas.height = 1;
                      const context = canvas.getContext("2d");
                      context.drawImage(image, 0, 0);
                      const pixels = Array.from(context.getImageData(0, 0, 1, 1).data);
                      const dataUrl = canvas.toDataURL();
                      return [
                        loaded,
                        image.complete,
                        image.naturalWidth,
                        image.naturalHeight,
                        pixels,
                        dataUrl.startsWith("data:image/png;base64,")
                      ];
                    })())
                    "#,
                )
                .unwrap();

            assert_eq!(result, "[true,true,1,1,[255,0,0,255],true]");
        });
    }

    fn with_context(callback: impl FnOnce(rquickjs::Ctx<'_>)) {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            init(&ctx).unwrap();
            callback(ctx);
        });
    }
}
