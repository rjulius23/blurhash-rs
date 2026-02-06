// BlurHash encode compute shader.
//
// Performs the DCT (Discrete Cosine Transform) summation for encoding.
// Each workgroup computes one DCT component (i, j) by summing over all pixels.
//
// Strategy: Each invocation handles a tile of rows. We use workgroup shared
// memory to reduce partial sums across the workgroup.

const PI: f32 = 3.14159265358979323846;

// Uniform parameters for the encode pass.
struct EncodeParams {
    width: u32,
    height: u32,
    components_x: u32,
    components_y: u32,
}

@group(0) @binding(0) var<uniform> params: EncodeParams;

// Input: linear RGB pixels as f32 triples, length = width * height * 3.
@group(0) @binding(1) var<storage, read> pixels: array<f32>;

// Output: DCT components as f32 triples, length = components_x * components_y * 3.
// Layout: [r0, g0, b0, r1, g1, b1, ...] for component index j * components_x + i.
@group(0) @binding(2) var<storage, read_write> components: array<f32>;

// Shared memory for parallel reduction within a workgroup.
// We reduce 3 channels simultaneously.
const WORKGROUP_SIZE: u32 = 256u;
var<workgroup> shared_r: array<f32, 256>;
var<workgroup> shared_g: array<f32, 256>;
var<workgroup> shared_b: array<f32, 256>;

@compute @workgroup_size(256, 1, 1)
fn encode_dct(
    @builtin(workgroup_id) wg_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    // Each workgroup handles one DCT component.
    let component_idx = wg_id.x;
    let i = component_idx % params.components_x;
    let j = component_idx / params.components_x;

    let wf = f32(params.width);
    let hf = f32(params.height);
    let total_pixels = params.width * params.height;
    let tid = local_id.x;

    // Normalization factor: 1.0 for DC, 2.0 for AC.
    let norm_factor = select(2.0, 1.0, i == 0u && j == 0u);

    // Each thread sums over a strided subset of all pixels.
    var local_r: f32 = 0.0;
    var local_g: f32 = 0.0;
    var local_b: f32 = 0.0;

    var pixel_idx = tid;
    loop {
        if pixel_idx >= total_pixels {
            break;
        }
        let x = pixel_idx % params.width;
        let y = pixel_idx / params.width;

        let cos_x = cos(PI * f32(i) * f32(x) / wf);
        let cos_y = cos(PI * f32(j) * f32(y) / hf);
        let basis = norm_factor * cos_x * cos_y;

        let base = pixel_idx * 3u;
        local_r += basis * pixels[base];
        local_g += basis * pixels[base + 1u];
        local_b += basis * pixels[base + 2u];

        pixel_idx += WORKGROUP_SIZE;
    }

    // Store local sums into shared memory.
    shared_r[tid] = local_r;
    shared_g[tid] = local_g;
    shared_b[tid] = local_b;
    workgroupBarrier();

    // Parallel reduction.
    var stride = WORKGROUP_SIZE / 2u;
    loop {
        if stride == 0u {
            break;
        }
        if tid < stride {
            shared_r[tid] += shared_r[tid + stride];
            shared_g[tid] += shared_g[tid + stride];
            shared_b[tid] += shared_b[tid + stride];
        }
        workgroupBarrier();
        stride = stride / 2u;
    }

    // Thread 0 writes the final result, scaled by 1 / (width * height).
    if tid == 0u {
        let scale = 1.0 / (wf * hf);
        let out_idx = component_idx * 3u;
        components[out_idx] = shared_r[0] * scale;
        components[out_idx + 1u] = shared_g[0] * scale;
        components[out_idx + 2u] = shared_b[0] * scale;
    }
}
