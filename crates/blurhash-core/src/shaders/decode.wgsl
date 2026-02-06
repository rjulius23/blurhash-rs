// BlurHash decode compute shader.
//
// Reconstructs an image from DCT components. Each invocation computes one
// output pixel by summing over all DCT components.

const PI: f32 = 3.14159265358979323846;

// Uniform parameters for the decode pass.
struct DecodeParams {
    width: u32,
    height: u32,
    components_x: u32,
    components_y: u32,
}

@group(0) @binding(0) var<uniform> params: DecodeParams;

// Input: DCT components as f32 triples (in linear RGB).
// Layout: [r0, g0, b0, r1, g1, b1, ...] for component index j * components_x + i.
@group(0) @binding(1) var<storage, read> colours: array<f32>;

// Output: linear RGB pixels as f32 triples, length = width * height * 3.
@group(0) @binding(2) var<storage, read_write> pixels: array<f32>;

@compute @workgroup_size(256, 1, 1)
fn decode_dct(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let pixel_idx = global_id.x;
    let total_pixels = params.width * params.height;
    if pixel_idx >= total_pixels {
        return;
    }

    let x = pixel_idx % params.width;
    let y = pixel_idx / params.width;
    let wf = f32(params.width);
    let hf = f32(params.height);

    var pixel_r: f32 = 0.0;
    var pixel_g: f32 = 0.0;
    var pixel_b: f32 = 0.0;

    for (var j: u32 = 0u; j < params.components_y; j++) {
        let cos_y = cos(PI * f32(y) * f32(j) / hf);
        for (var i: u32 = 0u; i < params.components_x; i++) {
            let cos_x = cos(PI * f32(x) * f32(i) / wf);
            let basis = cos_x * cos_y;
            let c_idx = (j * params.components_x + i) * 3u;
            pixel_r += colours[c_idx] * basis;
            pixel_g += colours[c_idx + 1u] * basis;
            pixel_b += colours[c_idx + 2u] * basis;
        }
    }

    let out_idx = pixel_idx * 3u;
    pixels[out_idx] = pixel_r;
    pixels[out_idx + 1u] = pixel_g;
    pixels[out_idx + 2u] = pixel_b;
}
