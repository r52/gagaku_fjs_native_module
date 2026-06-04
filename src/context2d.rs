use std::time::Instant;

use rquickjs::{Class, Ctx, Result, function::Opt};

use crate::canvas::{
    NativeCanvas, clip_copy_region, copy_object_region, copy_region_from_flipped_rows,
    copy_region_to_flipped_rows, f64_to_i32,
};
use crate::image::NativeImage;
use crate::image_data::{NativeImageData, image_data_class};
use crate::logging::profile_image_operation;
use crate::typed_array::{
    image_data_byte_length, new_uint8_clamped_array, with_object_bytes, with_object_bytes_mut,
};

#[derive(rquickjs::class::Trace, rquickjs::JsLifetime)]
#[rquickjs::class(rename = "CanvasRenderingContext2D")]
pub struct NativeCanvasRenderingContext2D<'js> {
    canvas: Class<'js, NativeCanvas<'js>>,
}

#[rquickjs::methods(rename_all = "camelCase")]
impl<'js> NativeCanvasRenderingContext2D<'js> {
    #[qjs(constructor)]
    pub fn new(ctx: Ctx<'js>) -> Result<Self> {
        let canvas = Class::instance(ctx, NativeCanvas::new())?;
        Ok(Self { canvas })
    }

    pub fn draw_image(
        &self,
        ctx: Ctx<'js>,
        image: Class<'js, NativeImage<'js>>,
        dx: f64,
        dy: f64,
        dw: Opt<f64>,
        dh: Opt<f64>,
    ) -> Result<()> {
        let started_at = Instant::now();
        let image = image.borrow();
        if !image.complete_value() {
            return Ok(());
        }

        let image_width = image.natural_width_value();
        let image_height = image.natural_height_value();
        let draw_width = dw.0.map(f64_to_i32).unwrap_or(image_width as i32);
        let draw_height = dh.0.map(f64_to_i32).unwrap_or(image_height as i32);
        let (canvas_width, canvas_height, canvas_pixels) = {
            let mut canvas = self.canvas.borrow_mut();
            (
                canvas.width_value(),
                canvas.height_value(),
                canvas.pixels_object(ctx.clone())?,
            )
        };

        let Some(region) = clip_copy_region(
            image_width,
            image_height,
            0,
            0,
            canvas_width,
            canvas_height,
            f64_to_i32(dx),
            f64_to_i32(dy),
            draw_width,
            draw_height,
        ) else {
            return Ok(());
        };

        copy_object_region(
            &ctx,
            &image.pixels_object(),
            image_width,
            &canvas_pixels,
            canvas_width,
            region,
        )?;
        profile_image_operation(
            &ctx,
            "CanvasRenderingContext2D.drawImage",
            started_at,
            format!("{}x{}", region.width, region.height),
        );
        Ok(())
    }

    pub fn get_image_data(
        &self,
        ctx: Ctx<'js>,
        sx: f64,
        sy: f64,
        sw: f64,
        sh: f64,
    ) -> Result<Class<'js, NativeImageData<'js>>> {
        let started_at = Instant::now();
        let width = f64_to_i32(sw);
        let height = f64_to_i32(sh);
        if width < 0 || height < 0 {
            return Err(rquickjs::Exception::throw_range(
                &ctx,
                "ImageData dimensions must be non-negative",
            ));
        }
        let width = width as u32;
        let height = height as u32;
        let pixels =
            new_uint8_clamped_array(ctx.clone(), image_data_byte_length(&ctx, width, height)?)?;
        let (canvas_width, canvas_height, canvas_pixels) = {
            let mut canvas = self.canvas.borrow_mut();
            (
                canvas.width_value(),
                canvas.height_value(),
                canvas.pixels_object(ctx.clone())?,
            )
        };

        if let Some(region) = clip_copy_region(
            canvas_width,
            canvas_height,
            f64_to_i32(sx),
            f64_to_i32(sy),
            width,
            height,
            0,
            0,
            width as i32,
            height as i32,
        ) {
            with_object_bytes(&ctx, &canvas_pixels, |source| {
                with_object_bytes_mut(&ctx, &pixels, |target| {
                    copy_region_to_flipped_rows(
                        source,
                        canvas_width,
                        target,
                        width,
                        height,
                        region,
                    );
                    Ok(())
                })
            })?;
        }

        let image_data = image_data_class(ctx.clone(), pixels, width, height)?;
        profile_image_operation(
            &ctx,
            "CanvasRenderingContext2D.getImageData",
            started_at,
            format!("{width}x{height}"),
        );
        Ok(image_data)
    }

    pub fn put_image_data(
        &self,
        ctx: Ctx<'js>,
        image_data: Class<'js, NativeImageData<'js>>,
        dx: f64,
        dy: f64,
    ) -> Result<()> {
        let started_at = Instant::now();
        let image_data = image_data.borrow();
        let image_width = image_data.width_value();
        let image_height = image_data.height_value();
        let (canvas_width, canvas_height, canvas_pixels) = {
            let mut canvas = self.canvas.borrow_mut();
            (
                canvas.width_value(),
                canvas.height_value(),
                canvas.pixels_object(ctx.clone())?,
            )
        };

        let Some(region) = clip_copy_region(
            image_width,
            image_height,
            0,
            0,
            canvas_width,
            canvas_height,
            f64_to_i32(dx),
            f64_to_i32(dy),
            image_width as i32,
            image_height as i32,
        ) else {
            return Ok(());
        };

        with_object_bytes(&ctx, &image_data.data_object(), |source| {
            with_object_bytes_mut(&ctx, &canvas_pixels, |target| {
                copy_region_from_flipped_rows(
                    source,
                    image_width,
                    image_height,
                    target,
                    canvas_width,
                    region,
                );
                Ok(())
            })
        })?;
        profile_image_operation(
            &ctx,
            "CanvasRenderingContext2D.putImageData",
            started_at,
            format!("{}x{}", region.width, region.height),
        );
        Ok(())
    }
}

impl<'js> NativeCanvasRenderingContext2D<'js> {
    pub fn new_for_canvas(
        ctx: Ctx<'js>,
        canvas: Class<'js, NativeCanvas<'js>>,
    ) -> Result<Class<'js, Self>> {
        Class::instance(ctx, Self { canvas })
    }
}
