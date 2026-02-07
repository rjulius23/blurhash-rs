//! GPU-accelerated BlurHash encoding and decoding via wgpu compute shaders.
//!
//! This module provides `encode_gpu()` and `decode_gpu()` functions that
//! offload the DCT computation to the GPU using wgpu. If no GPU is available,
//! they fall back to the CPU implementations transparently.
//!
//! # Requirements
//!
//! Enable the `gpu` feature in Cargo.toml:
//!
//! ```toml
//! blurhash-core = { version = "0.1", features = ["gpu"] }
//! ```
//!
//! # Performance Notes
//!
//! GPU acceleration is most beneficial for:
//! - **Encoding** large images (>256x256) where the O(W*H*Cx*Cy) DCT dominates
//! - **Decoding** to large output dimensions (>256x256)
//! - Batch processing multiple images (amortizes GPU setup cost)
//!
//! For small images (<64x64), CPU is typically faster due to GPU dispatch overhead.

use std::sync::OnceLock;

use wgpu::util::DeviceExt;

use crate::base83;
use crate::color::{linear_to_srgb, sign_pow, srgb_to_linear};
use crate::error::BlurhashError;

/// Holds initialized wgpu device, queue, and compiled pipelines.
struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    encode_pipeline: wgpu::ComputePipeline,
    decode_pipeline: wgpu::ComputePipeline,
    encode_bind_group_layout: wgpu::BindGroupLayout,
    decode_bind_group_layout: wgpu::BindGroupLayout,
}

/// Global lazily-initialized GPU context.
static GPU_CONTEXT: OnceLock<Option<GpuContext>> = OnceLock::new();

/// Workgroup size must match the WGSL shaders.
const WORKGROUP_SIZE: u32 = 256;

/// Minimum total pixels (width * height) before GPU dispatch is worthwhile.
/// Below this threshold, CPU fallback is used automatically.
const GPU_PIXEL_THRESHOLD: u32 = 64 * 64;

/// Initialize the GPU context (device, queue, pipelines).
/// Returns `None` if no suitable GPU adapter is found.
fn init_gpu() -> Option<GpuContext> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))?;

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("blurhash-gpu"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::Performance,
        },
        None,
    ))
    .ok()?;

    // Compile encode shader.
    let encode_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("blurhash-encode"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/encode.wgsl").into()),
    });

    // Compile decode shader.
    let decode_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("blurhash-decode"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/decode.wgsl").into()),
    });

    // Bind group layout shared by both encode and decode:
    // binding 0: uniform params
    // binding 1: storage read (input)
    // binding 2: storage read_write (output)
    let encode_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("encode-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

    let decode_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("decode-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

    let encode_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("encode-pl"),
        bind_group_layouts: &[&encode_bind_group_layout],
        push_constant_ranges: &[],
    });

    let decode_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("decode-pl"),
        bind_group_layouts: &[&decode_bind_group_layout],
        push_constant_ranges: &[],
    });

    let encode_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("encode-pipeline"),
        layout: Some(&encode_pipeline_layout),
        module: &encode_shader,
        entry_point: Some("encode_dct"),
        compilation_options: Default::default(),
        cache: None,
    });

    let decode_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("decode-pipeline"),
        layout: Some(&decode_pipeline_layout),
        module: &decode_shader,
        entry_point: Some("decode_dct"),
        compilation_options: Default::default(),
        cache: None,
    });

    Some(GpuContext {
        device,
        queue,
        encode_pipeline,
        decode_pipeline,
        encode_bind_group_layout,
        decode_bind_group_layout,
    })
}

/// Get or initialize the global GPU context.
fn get_gpu() -> Option<&'static GpuContext> {
    GPU_CONTEXT.get_or_init(|| init_gpu()).as_ref()
}

/// Returns `true` if a GPU is available for acceleration.
pub fn gpu_available() -> bool {
    get_gpu().is_some()
}

/// Encode an RGB image into a BlurHash string using GPU acceleration.
///
/// Falls back to CPU if GPU is unavailable or the image is too small to
/// benefit from GPU dispatch.
///
/// Arguments and behavior are identical to [`crate::encode`].
pub fn encode_gpu(
    pixels: &[u8],
    width: u32,
    height: u32,
    components_x: u32,
    components_y: u32,
) -> Result<String, BlurhashError> {
    // Validate inputs (same as CPU path).
    if width == 0 || height == 0 {
        return Err(BlurhashError::InvalidDimensions {
            width,
            height,
            reason: "width and height must be > 0",
        });
    }
    const MAX_DIMENSION: u32 = 10_000;
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(BlurhashError::InvalidDimensions {
            width,
            height,
            reason: "dimensions must be <= 10000",
        });
    }
    if !(1..=9).contains(&components_x) {
        return Err(BlurhashError::InvalidComponentCount {
            component: "x",
            value: components_x,
        });
    }
    if !(1..=9).contains(&components_y) {
        return Err(BlurhashError::InvalidComponentCount {
            component: "y",
            value: components_y,
        });
    }
    let expected_len = (width as usize) * (height as usize) * 3;
    if pixels.len() != expected_len {
        return Err(BlurhashError::EncodingError(format!(
            "pixel buffer length {} does not match {}x{}x3 = {}",
            pixels.len(),
            width,
            height,
            expected_len
        )));
    }

    // Fall back to CPU for small images or if GPU is unavailable.
    let total_pixels = width * height;
    let gpu = match get_gpu() {
        Some(ctx) if total_pixels >= GPU_PIXEL_THRESHOLD => ctx,
        _ => return crate::encode(pixels, width, height, components_x, components_y),
    };

    // Convert sRGB to linear f32 on CPU (fast LUT lookup).
    let linear_pixels: Vec<f32> = pixels
        .chunks_exact(3)
        .flat_map(|rgb| {
            [
                srgb_to_linear(rgb[0]) as f32,
                srgb_to_linear(rgb[1]) as f32,
                srgb_to_linear(rgb[2]) as f32,
            ]
        })
        .collect();

    let num_components = (components_x * components_y) as usize;

    // Create GPU buffers.
    let params_data = [width, height, components_x, components_y];
    let params_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("encode-params"),
            contents: bytemuck::cast_slice(&params_data),
            usage: wgpu::BufferUsages::UNIFORM,
        });

    let pixel_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("encode-pixels"),
            contents: bytemuck::cast_slice(&linear_pixels),
            usage: wgpu::BufferUsages::STORAGE,
        });

    let components_size = (num_components * 3 * std::mem::size_of::<f32>()) as u64;
    let components_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("encode-components"),
        size: components_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let readback_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("encode-readback"),
        size: components_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Create bind group.
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("encode-bg"),
        layout: &gpu.encode_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: pixel_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: components_buf.as_entire_binding(),
            },
        ],
    });

    // Dispatch: one workgroup per DCT component.
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("encode-cmd"),
        });

    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("encode-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&gpu.encode_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(num_components as u32, 1, 1);
    }

    encoder.copy_buffer_to_buffer(&components_buf, 0, &readback_buf, 0, components_size);
    gpu.queue.submit(std::iter::once(encoder.finish()));

    // Read back results.
    let components_slice = readback_buf.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    components_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    gpu.device.poll(wgpu::Maintain::Wait);
    rx.recv()
        .map_err(|e| BlurhashError::EncodingError(format!("GPU readback failed: {e}")))?
        .map_err(|e| BlurhashError::EncodingError(format!("GPU buffer map failed: {e}")))?;

    let data = components_slice.get_mapped_range();
    let gpu_components: &[f32] = bytemuck::cast_slice(&data);

    // Convert GPU f32 components to the same quantized base83 format as CPU path.
    let dc_r = gpu_components[0] as f64;
    let dc_g = gpu_components[1] as f64;
    let dc_b = gpu_components[2] as f64;

    let dc_value = ((linear_to_srgb(dc_r) as u64) << 16)
        | ((linear_to_srgb(dc_g) as u64) << 8)
        | (linear_to_srgb(dc_b) as u64);

    // Find max AC component magnitude.
    let mut max_ac_component: f64 = 0.0;
    for chunk in gpu_components[3..].chunks_exact(3) {
        max_ac_component = max_ac_component
            .max((chunk[0] as f64).abs())
            .max((chunk[1] as f64).abs())
            .max((chunk[2] as f64).abs());
    }

    let quant_max_ac = (max_ac_component * 166.0 - 0.5).floor().clamp(0.0, 82.0) as u64;
    let ac_component_norm_factor = (quant_max_ac as f64 + 1.0) / 166.0;

    // Quantize AC values.
    let mut ac_values: Vec<u64> = Vec::with_capacity(num_components - 1);
    for chunk in gpu_components[3..].chunks_exact(3) {
        let quant_r = (sign_pow(chunk[0] as f64 / ac_component_norm_factor, 0.5) * 9.0 + 9.5)
            .floor()
            .clamp(0.0, 18.0) as u64;
        let quant_g = (sign_pow(chunk[1] as f64 / ac_component_norm_factor, 0.5) * 9.0 + 9.5)
            .floor()
            .clamp(0.0, 18.0) as u64;
        let quant_b = (sign_pow(chunk[2] as f64 / ac_component_norm_factor, 0.5) * 9.0 + 9.5)
            .floor()
            .clamp(0.0, 18.0) as u64;
        ac_values.push(quant_r * 19 * 19 + quant_g * 19 + quant_b);
    }

    drop(data);
    readback_buf.unmap();

    // Build the BlurHash string.
    let size_flag = (components_x - 1) + (components_y - 1) * 9;
    let estimated_len = 4 + 2 * num_components;
    let mut result = String::with_capacity(estimated_len);

    result.push_str(&base83::encode(size_flag as u64, 1)?);
    result.push_str(&base83::encode(quant_max_ac, 1)?);
    result.push_str(&base83::encode(dc_value, 4)?);
    for ac_value in &ac_values {
        result.push_str(&base83::encode(*ac_value, 2)?);
    }

    Ok(result)
}

/// Decode a BlurHash string into a flat RGB byte array using GPU acceleration.
///
/// Falls back to CPU if GPU is unavailable or the output image is too small
/// to benefit from GPU dispatch.
///
/// Arguments and behavior are identical to [`crate::decode`].
pub fn decode_gpu(
    blurhash: &str,
    width: u32,
    height: u32,
    punch: f64,
) -> Result<Vec<u8>, BlurhashError> {
    // Validate inputs (same as CPU path).
    if width == 0 || height == 0 {
        return Err(BlurhashError::InvalidDimensions {
            width,
            height,
            reason: "width and height must be > 0",
        });
    }
    const MAX_DIMENSION: u32 = 10_000;
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(BlurhashError::InvalidDimensions {
            width,
            height,
            reason: "dimensions must be <= 10000",
        });
    }

    if blurhash.len() < 6 {
        return Err(BlurhashError::InvalidLength {
            expected: 6,
            actual: blurhash.len(),
        });
    }

    let size_info = base83::decode(&blurhash[0..1])?;
    let size_y = (size_info / 9) + 1;
    let size_x = (size_info % 9) + 1;
    let components_x = size_x as u32;
    let components_y = size_y as u32;

    let expected_len = 4 + 2 * (size_x * size_y) as usize;
    if blurhash.len() != expected_len {
        return Err(BlurhashError::InvalidLength {
            expected: expected_len,
            actual: blurhash.len(),
        });
    }

    // Fall back to CPU for small outputs or if GPU is unavailable.
    let total_pixels = width * height;
    let gpu = match get_gpu() {
        Some(ctx) if total_pixels >= GPU_PIXEL_THRESHOLD => ctx,
        _ => return crate::decode(blurhash, width, height, punch),
    };

    // Parse BlurHash components on CPU (trivially fast).
    let quant_max_value = base83::decode(&blurhash[1..2])?;
    let real_max_value = (quant_max_value as f64 + 1.0) / 166.0 * punch;

    let dc_value = base83::decode(&blurhash[2..6])?;
    let dc_r = srgb_to_linear(((dc_value >> 16) & 255) as u8);
    let dc_g = srgb_to_linear(((dc_value >> 8) & 255) as u8);
    let dc_b = srgb_to_linear((dc_value & 255) as u8);

    let num_components = (size_x * size_y) as usize;
    let mut colours: Vec<f32> = Vec::with_capacity(num_components * 3);
    colours.extend_from_slice(&[dc_r as f32, dc_g as f32, dc_b as f32]);

    for component_idx in 1..num_components {
        let start = 4 + component_idx * 2;
        let ac_value = base83::decode(&blurhash[start..start + 2])?;

        let quant_r = (ac_value / (19 * 19)) as f64;
        let quant_g = ((ac_value / 19) % 19) as f64;
        let quant_b = (ac_value % 19) as f64;

        colours.push((sign_pow((quant_r - 9.0) / 9.0, 2.0) * real_max_value) as f32);
        colours.push((sign_pow((quant_g - 9.0) / 9.0, 2.0) * real_max_value) as f32);
        colours.push((sign_pow((quant_b - 9.0) / 9.0, 2.0) * real_max_value) as f32);
    }

    // Create GPU buffers.
    let params_data = [width, height, components_x, components_y];
    let params_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("decode-params"),
            contents: bytemuck::cast_slice(&params_data),
            usage: wgpu::BufferUsages::UNIFORM,
        });

    let colours_buf = gpu
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("decode-colours"),
            contents: bytemuck::cast_slice(&colours),
            usage: wgpu::BufferUsages::STORAGE,
        });

    let pixels_size = (total_pixels as usize * 3 * std::mem::size_of::<f32>()) as u64;
    let pixels_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("decode-pixels"),
        size: pixels_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let readback_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("decode-readback"),
        size: pixels_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Create bind group.
    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("decode-bg"),
        layout: &gpu.decode_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: colours_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: pixels_buf.as_entire_binding(),
            },
        ],
    });

    // Dispatch: one thread per output pixel.
    let num_workgroups = (total_pixels + WORKGROUP_SIZE - 1) / WORKGROUP_SIZE;
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("decode-cmd"),
        });

    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("decode-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&gpu.decode_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(num_workgroups, 1, 1);
    }

    encoder.copy_buffer_to_buffer(&pixels_buf, 0, &readback_buf, 0, pixels_size);
    gpu.queue.submit(std::iter::once(encoder.finish()));

    // Read back results.
    let pixels_slice = readback_buf.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    pixels_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    gpu.device.poll(wgpu::Maintain::Wait);
    rx.recv()
        .map_err(|e| BlurhashError::EncodingError(format!("GPU readback failed: {e}")))?
        .map_err(|e| BlurhashError::EncodingError(format!("GPU buffer map failed: {e}")))?;

    let data = pixels_slice.get_mapped_range();
    let gpu_pixels: &[f32] = bytemuck::cast_slice(&data);

    // Convert linear f32 back to sRGB u8 on CPU.
    let w = width as usize;
    let h = height as usize;
    let mut result = vec![0u8; w * h * 3];
    for (i, chunk) in gpu_pixels.chunks_exact(3).enumerate() {
        let idx = i * 3;
        result[idx] = linear_to_srgb(chunk[0] as f64);
        result[idx + 1] = linear_to_srgb(chunk[1] as f64);
        result[idx + 2] = linear_to_srgb(chunk[2] as f64);
    }

    drop(data);
    readback_buf.unmap();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_available_does_not_panic() {
        // Just ensure initialization doesn't panic; availability depends on hardware.
        let _ = gpu_available();
    }

    #[test]
    fn test_encode_gpu_fallback_small_image() {
        // Small image should fall back to CPU and still produce correct results.
        let pixels = vec![128u8; 4 * 4 * 3];
        let gpu_hash = encode_gpu(&pixels, 4, 4, 4, 3).unwrap();
        let cpu_hash = crate::encode(&pixels, 4, 4, 4, 3).unwrap();
        assert_eq!(gpu_hash, cpu_hash);
    }

    #[test]
    fn test_decode_gpu_fallback_small_image() {
        let hash = "LEHV6nWB2yk8pyo0adR*.7kCMdnj";
        let gpu_pixels = decode_gpu(hash, 32, 32, 1.0).unwrap();
        let cpu_pixels = crate::decode(hash, 32, 32, 1.0).unwrap();
        assert_eq!(gpu_pixels, cpu_pixels);
    }

    #[test]
    fn test_encode_gpu_validation() {
        let pixels = vec![0u8; 4 * 4 * 3];
        assert!(encode_gpu(&pixels, 0, 4, 4, 3).is_err());
        assert!(encode_gpu(&pixels, 4, 4, 10, 3).is_err());
        assert!(encode_gpu(&pixels, 4, 4, 4, 0).is_err());
    }

    #[test]
    fn test_decode_gpu_validation() {
        assert!(decode_gpu("ABC", 32, 32, 1.0).is_err());
        assert!(decode_gpu("LEHV6nWB2yk8pyo0adR*.7kCMdnj", 0, 32, 1.0).is_err());
    }

    #[test]
    fn test_encode_decode_gpu_roundtrip() {
        // Test with a larger image if GPU is available, or falls back to CPU.
        let w = 128;
        let h = 128;
        let mut pixels = vec![0u8; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 3;
                pixels[idx] = (x * 2).min(255) as u8;
                pixels[idx + 1] = (y * 2).min(255) as u8;
                pixels[idx + 2] = 128;
            }
        }
        let hash = encode_gpu(&pixels, w as u32, h as u32, 4, 3).unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 4 + 2 * 4 * 3);

        let decoded = decode_gpu(&hash, 32, 32, 1.0).unwrap();
        assert_eq!(decoded.len(), 32 * 32 * 3);
    }
}
