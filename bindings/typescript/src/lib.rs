use napi::bindgen_prelude::*;
use napi::Task;
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

/// Encode from a Uint8Array (for browser/Deno compatibility).
///
/// @param data - Raw pixel bytes as Uint8Array in RGB order.
/// @param width - Image width in pixels.
/// @param height - Image height in pixels.
/// @param components_x - Number of horizontal components (1..=9, default 4).
/// @param components_y - Number of vertical components (1..=9, default 4).
/// @returns The BlurHash string.
#[napi]
pub fn encode_from_uint8_array(
    data: Uint8Array,
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

/// Decode a BlurHash string into a Uint8Array (for browser/Deno compatibility).
///
/// @param blurhash - The BlurHash string to decode.
/// @param width - Desired output width in pixels.
/// @param height - Desired output height in pixels.
/// @param punch - Contrast adjustment factor (default 1.0).
/// @returns A Uint8Array of length width * height * 3 containing RGB pixel data.
#[napi]
pub fn decode_to_uint8_array(
    blurhash: String,
    width: u32,
    height: u32,
    punch: Option<f64>,
) -> Result<Uint8Array> {
    let p = punch.unwrap_or(1.0);
    let pixels = blurhash_core::decode(&blurhash, width, height, p)
        .map_err(|e| Error::from_reason(e.to_string()))?;
    Ok(Uint8Array::from(pixels))
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

// --- Async versions (run on libuv thread pool) ---

pub struct EncodeTask {
    data: Vec<u8>,
    width: u32,
    height: u32,
    components_x: u32,
    components_y: u32,
}

impl Task for EncodeTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> Result<Self::Output> {
        blurhash_core::encode(&self.data, self.width, self.height, self.components_x, self.components_y)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

/// Async version of encode that runs on the libuv thread pool.
/// Returns a Promise<string>.
///
/// @param data - Raw pixel bytes in RGB order (length must be width * height * 3).
/// @param width - Image width in pixels.
/// @param height - Image height in pixels.
/// @param components_x - Number of horizontal components (1..=9, default 4).
/// @param components_y - Number of vertical components (1..=9, default 4).
/// @returns A Promise resolving to the BlurHash string.
#[napi]
pub fn encode_async(
    data: Buffer,
    width: u32,
    height: u32,
    components_x: Option<u32>,
    components_y: Option<u32>,
) -> AsyncTask<EncodeTask> {
    let cx = components_x.unwrap_or(4);
    let cy = components_y.unwrap_or(4);
    AsyncTask::new(EncodeTask {
        data: data.to_vec(),
        width,
        height,
        components_x: cx,
        components_y: cy,
    })
}

pub struct DecodeTask {
    blurhash: String,
    width: u32,
    height: u32,
    punch: f64,
}

impl Task for DecodeTask {
    type Output = Vec<u8>;
    type JsValue = Buffer;

    fn compute(&mut self) -> Result<Self::Output> {
        blurhash_core::decode(&self.blurhash, self.width, self.height, self.punch)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(Buffer::from(output))
    }
}

/// Async version of decode that runs on the libuv thread pool.
/// Returns a Promise<Buffer>.
///
/// @param blurhash - The BlurHash string to decode.
/// @param width - Desired output width in pixels.
/// @param height - Desired output height in pixels.
/// @param punch - Contrast adjustment factor (default 1.0).
/// @returns A Promise resolving to a Buffer of RGB pixel data.
#[napi]
pub fn decode_async(
    blurhash: String,
    width: u32,
    height: u32,
    punch: Option<f64>,
) -> AsyncTask<DecodeTask> {
    let p = punch.unwrap_or(1.0);
    AsyncTask::new(DecodeTask {
        blurhash,
        width,
        height,
        punch: p,
    })
}
