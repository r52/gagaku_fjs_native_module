use rquickjs::{Class, Ctx, Exception, Object, Result, Value};

use crate::typed_array::{
    image_data_byte_length, new_uint8_clamped_array, value_as_u32, with_object_bytes,
};

#[derive(rquickjs::class::Trace, rquickjs::JsLifetime)]
#[rquickjs::class(rename = "ImageData")]
pub struct NativeImageData<'js> {
    data: Object<'js>,
    width: u32,
    height: u32,
}

#[rquickjs::methods]
impl<'js> NativeImageData<'js> {
    #[qjs(constructor)]
    pub fn new(
        ctx: Ctx<'js>,
        data_or_width: Value<'js>,
        width_or_height: u32,
        height: rquickjs::function::Opt<u32>,
    ) -> Result<Self> {
        let (data, width, height) = if let Some(width) = value_as_u32(&data_or_width) {
            let height = width_or_height;
            let byte_length = image_data_byte_length(&ctx, width, height)?;
            (
                new_uint8_clamped_array(ctx.clone(), byte_length)?,
                width,
                height,
            )
        } else {
            let data = data_or_width.as_object().cloned().ok_or_else(|| {
                Exception::throw_type(&ctx, "ImageData data must be a Uint8ClampedArray")
            })?;
            let width = width_or_height;
            let byte_length: u32 = data.get("byteLength")?;
            let resolved_height = match height.0 {
                Some(height) => height,
                None => {
                    let row_bytes = width.checked_mul(4).ok_or_else(|| {
                        Exception::throw_range(&ctx, "ImageData row byte length overflow")
                    })?;
                    if row_bytes == 0 || byte_length % row_bytes != 0 {
                        return Err(Exception::throw_range(
                            &ctx,
                            "ImageData pixel length does not match width",
                        ));
                    }
                    byte_length / row_bytes
                }
            };
            let expected_len = image_data_byte_length(&ctx, width, resolved_height)?;
            if byte_length != expected_len {
                return Err(Exception::throw_range(
                    &ctx,
                    "ImageData pixel length does not match dimensions",
                ));
            }
            (data, width, resolved_height)
        };

        Ok(Self {
            data,
            width,
            height,
        })
    }

    #[qjs(get)]
    pub fn data(&self) -> Object<'js> {
        self.data.clone()
    }

    #[qjs(get)]
    pub fn width(&self) -> u32 {
        self.width
    }

    #[qjs(get)]
    pub fn height(&self) -> u32 {
        self.height
    }

    #[qjs(get, rename = "colorSpace")]
    pub fn color_space(&self) -> &'static str {
        "srgb"
    }
}

impl<'js> NativeImageData<'js> {
    pub fn from_data(data: Object<'js>, width: u32, height: u32) -> Self {
        Self {
            data,
            width,
            height,
        }
    }

    pub fn data_object(&self) -> Object<'js> {
        self.data.clone()
    }

    pub fn width_value(&self) -> u32 {
        self.width
    }

    pub fn height_value(&self) -> u32 {
        self.height
    }
}

pub fn inspect_image_data<'js>(ctx: Ctx<'js>, image_data: Value<'js>) -> Result<Object<'js>> {
    let image_data = image_data.as_object().ok_or_else(|| {
        Exception::throw_type(&ctx, "inspectImageData expects an ImageData object")
    })?;
    let data: Object<'js> = image_data.get("data")?;
    with_object_bytes(&ctx, &data, |bytes| {
        let result = Object::new(ctx.clone())?;
        result.set("width", image_data.get::<_, u32>("width")?)?;
        result.set("height", image_data.get::<_, u32>("height")?)?;
        result.set("length", bytes.len())?;
        result.set(
            "checksum",
            bytes.iter().map(|byte| u32::from(*byte)).sum::<u32>(),
        )?;
        result.set("first", bytes.first().copied().unwrap_or_default())?;
        result.set("second", bytes.get(1).copied().unwrap_or_default())?;
        result.set("third", bytes.get(2).copied().unwrap_or_default())?;
        result.set("fourth", bytes.get(3).copied().unwrap_or_default())?;
        Ok(result)
    })
}

pub fn image_data_class<'js>(
    ctx: Ctx<'js>,
    data: Object<'js>,
    width: u32,
    height: u32,
) -> Result<Class<'js, NativeImageData<'js>>> {
    Class::instance(ctx, NativeImageData::from_data(data, width, height))
}
