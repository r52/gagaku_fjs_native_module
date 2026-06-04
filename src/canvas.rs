use std::time::Instant;

use rquickjs::{Class, Ctx, Object, Result, function::This};

use crate::context2d::NativeCanvasRenderingContext2D;
use crate::logging::{log_image_error, profile_image_operation};
use crate::png::encode_png_data_url_from_rgba;
use crate::typed_array::{
    image_data_byte_length, new_uint8_clamped_array, with_object_bytes, with_object_bytes_mut,
};

#[derive(rquickjs::class::Trace, rquickjs::JsLifetime)]
#[rquickjs::class(rename = "HTMLCanvasElement")]
pub struct NativeCanvas<'js> {
    width: u32,
    height: u32,
    context: Option<Class<'js, NativeCanvasRenderingContext2D<'js>>>,
    pixels: Option<Object<'js>>,
}

#[rquickjs::methods]
impl<'js> NativeCanvas<'js> {
    #[qjs(constructor)]
    pub fn new() -> Self {
        Self {
            width: 300,
            height: 150,
            context: None,
            pixels: None,
        }
    }

    #[qjs(get)]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[qjs(set, rename = "width")]
    pub fn set_width(&mut self, value: f64) {
        let width = f64_to_canvas_dimension(value);
        if width != self.width {
            self.width = width;
            self.pixels = None;
        }
    }

    #[qjs(get)]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[qjs(set, rename = "height")]
    pub fn set_height(&mut self, value: f64) {
        let height = f64_to_canvas_dimension(value);
        if height != self.height {
            self.height = height;
            self.pixels = None;
        }
    }

    #[qjs(get)]
    pub fn pixels(&mut self, ctx: Ctx<'js>) -> Result<Object<'js>> {
        self.pixels_object(ctx)
    }

    #[qjs(set, rename = "pixels")]
    pub fn set_pixels(&mut self, value: Object<'js>) {
        self.pixels = Some(value);
    }

    #[qjs(rename = "getContext")]
    pub fn get_context(
        ctx: Ctx<'js>,
        this: This<Class<'js, Self>>,
        context_id: String,
    ) -> Result<Option<Class<'js, NativeCanvasRenderingContext2D<'js>>>> {
        if context_id != "2d" {
            return Ok(None);
        }

        let canvas_class = this.0;
        if let Some(context) = canvas_class.borrow().context.clone() {
            return Ok(Some(context));
        }

        let context = NativeCanvasRenderingContext2D::new_for_canvas(ctx, canvas_class.clone())?;
        canvas_class.borrow_mut().context = Some(context.clone());
        Ok(Some(context))
    }

    #[qjs(rename = "toDataURL")]
    pub fn to_data_url(ctx: Ctx<'js>, this: This<Class<'js, Self>>) -> Result<String> {
        let started_at = Instant::now();
        let (pixels, width, height) = {
            let mut canvas = this.0.borrow_mut();
            (
                canvas.pixels_object(ctx.clone())?,
                canvas.width,
                canvas.height,
            )
        };

        let result = with_object_bytes(&ctx, &pixels, |bytes| {
            encode_png_data_url_from_rgba(bytes, width, height)
                .map_err(|err| rquickjs::Exception::throw_message(&ctx, &err.to_string()))
        });

        match &result {
            Ok(data_url) => profile_image_operation(
                &ctx,
                "HTMLCanvasElement.toDataURL",
                started_at,
                format!("{width}x{height} {} chars", data_url.len()),
            ),
            Err(err) => log_image_error(
                &ctx,
                "HTMLCanvasElement.toDataURL",
                started_at,
                format!("{width}x{height} {err}"),
            ),
        }

        result
    }
}

impl<'js> NativeCanvas<'js> {
    pub fn width_value(&self) -> u32 {
        self.width
    }

    pub fn height_value(&self) -> u32 {
        self.height
    }

    pub fn pixels_object(&mut self, ctx: Ctx<'js>) -> Result<Object<'js>> {
        if let Some(pixels) = &self.pixels {
            return Ok(pixels.clone());
        }

        let byte_length = image_data_byte_length(&ctx, self.width, self.height)?;
        let pixels = new_uint8_clamped_array(ctx, byte_length)?;
        self.pixels = Some(pixels.clone());
        Ok(pixels)
    }
}

pub fn f64_to_i32(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    value.trunc().clamp(i32::MIN as f64, i32::MAX as f64) as i32
}

fn f64_to_canvas_dimension(value: f64) -> u32 {
    if !value.is_finite() {
        return 0;
    }
    value.trunc().max(0.0).min(u32::MAX as f64) as u32
}

#[derive(Debug, Clone, Copy)]
pub struct CopyRegion {
    pub source_x: usize,
    pub source_y: usize,
    pub target_x: usize,
    pub target_y: usize,
    pub width: usize,
    pub height: usize,
}

pub fn clip_copy_region(
    source_width: u32,
    source_height: u32,
    source_x: i32,
    source_y: i32,
    target_width: u32,
    target_height: u32,
    target_x: i32,
    target_y: i32,
    width: i32,
    height: i32,
) -> Option<CopyRegion> {
    let mut source_x = source_x;
    let mut source_y = source_y;
    let mut target_x = target_x;
    let mut target_y = target_y;
    let mut width = width;
    let mut height = height;

    if source_x < 0 {
        width += source_x;
        target_x -= source_x;
        source_x = 0;
    }
    if source_y < 0 {
        height += source_y;
        target_y -= source_y;
        source_y = 0;
    }
    if target_x < 0 {
        width += target_x;
        source_x -= target_x;
        target_x = 0;
    }
    if target_y < 0 {
        height += target_y;
        source_y -= target_y;
        target_y = 0;
    }

    width = width
        .min(source_width as i32 - source_x)
        .min(target_width as i32 - target_x);
    height = height
        .min(source_height as i32 - source_y)
        .min(target_height as i32 - target_y);

    if width <= 0 || height <= 0 {
        return None;
    }

    Some(CopyRegion {
        source_x: source_x as usize,
        source_y: source_y as usize,
        target_x: target_x as usize,
        target_y: target_y as usize,
        width: width as usize,
        height: height as usize,
    })
}

pub fn copy_region(
    source: &[u8],
    source_width: u32,
    target: &mut [u8],
    target_width: u32,
    region: CopyRegion,
) {
    let row_bytes = region.width * 4;
    for y in 0..region.height {
        let source_offset = ((region.source_y + y) * source_width as usize + region.source_x) * 4;
        let target_offset = ((region.target_y + y) * target_width as usize + region.target_x) * 4;
        target[target_offset..target_offset + row_bytes]
            .copy_from_slice(&source[source_offset..source_offset + row_bytes]);
    }
}

pub fn copy_region_to_flipped_rows(
    source: &[u8],
    source_width: u32,
    target: &mut [u8],
    target_width: u32,
    target_height: u32,
    region: CopyRegion,
) {
    let row_bytes = region.width * 4;
    for y in 0..region.height {
        let source_offset = ((region.source_y + y) * source_width as usize + region.source_x) * 4;
        let flipped_target_y = target_height as usize - 1 - (region.target_y + y);
        let target_offset = (flipped_target_y * target_width as usize + region.target_x) * 4;
        target[target_offset..target_offset + row_bytes]
            .copy_from_slice(&source[source_offset..source_offset + row_bytes]);
    }
}

pub fn copy_region_from_flipped_rows(
    source: &[u8],
    source_width: u32,
    source_height: u32,
    target: &mut [u8],
    target_width: u32,
    region: CopyRegion,
) {
    let row_bytes = region.width * 4;
    for y in 0..region.height {
        let flipped_source_y = source_height as usize - 1 - (region.source_y + y);
        let source_offset = (flipped_source_y * source_width as usize + region.source_x) * 4;
        let target_offset = ((region.target_y + y) * target_width as usize + region.target_x) * 4;
        target[target_offset..target_offset + row_bytes]
            .copy_from_slice(&source[source_offset..source_offset + row_bytes]);
    }
}

pub fn copy_object_region<'js>(
    ctx: &Ctx<'js>,
    source: &Object<'js>,
    source_width: u32,
    target: &Object<'js>,
    target_width: u32,
    region: CopyRegion,
) -> Result<()> {
    with_object_bytes(ctx, source, |source| {
        with_object_bytes_mut(ctx, target, |target| {
            copy_region(source, source_width, target, target_width, region);
            Ok(())
        })
    })
}
