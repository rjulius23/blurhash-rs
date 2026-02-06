use napi::bindgen_prelude::*;
use napi_derive::napi;

/// Encode image pixel data into a BlurHash string.
///
/// @param data - Raw pixel bytes in RGB order (length must be width * height * 3).
/// @param width - Image width in pixels.
/// @param height - Image height in pixels.
/// @param components_x - Number of horizontal components (1..=9, default 4).
/// @param components_y - Number of vertical components (1..=9, default 4).
/// @returns The BlurHash string.
#[napi]
pub fn encode(
    data: Buffer,
    width: u32,
    height: u32,
    components_x: Option<u32>,
    components_y: Option<u32>,
) -> Result<String> {
    let cx = components_x.unwrap_or(4);
    let cy = components_y.unwrap_or(4);
    blurhash_core::encode(data.as_ref(), width, height, cx, cy)
        .map_err(|e| Error::from_reason(e.to_string()))
}

/// Decode a BlurHash string into raw RGB pixel data.
///
/// @param blurhash - The BlurHash string to decode.
/// @param width - Desired output width in pixels.
/// @param height - Desired output height in pixels.
/// @param punch - Contrast adjustment factor (default 1.0).
/// @returns A Buffer of length width * height * 3 containing RGB pixel data.
#[napi]
pub fn decode(
    blurhash: String,
    width: u32,
    height: u32,
    punch: Option<f64>,
) -> Result<Buffer> {
    let p = punch.unwrap_or(1.0);
    let pixels = blurhash_core::decode(&blurhash, width, height, p)
        .map_err(|e| Error::from_reason(e.to_string()))?;
    Ok(Buffer::from(pixels))
}

/// Extract the number of X and Y components from a BlurHash string.
///
/// @param blurhash - The BlurHash string.
/// @returns An object with componentsX and componentsY fields.
#[napi(object)]
pub struct Components {
    pub components_x: u32,
    pub components_y: u32,
}

#[napi]
pub fn get_components(blurhash: String) -> Result<Components> {
    let (cx, cy) = blurhash_core::components(&blurhash)
        .map_err(|e| Error::from_reason(e.to_string()))?;
    Ok(Components {
        components_x: cx,
        components_y: cy,
    })
}

/// Convert an sRGB byte value (0-255) to linear RGB (0.0-1.0).
#[napi]
pub fn srgb_to_linear(value: u8) -> f64 {
    blurhash_core::color::srgb_to_linear(value)
}

/// Convert a linear RGB value (0.0-1.0) to an sRGB byte value (0-255).
#[napi]
pub fn linear_to_srgb(value: f64) -> u8 {
    blurhash_core::color::linear_to_srgb(value)
}
