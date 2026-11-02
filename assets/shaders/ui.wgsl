struct UiUniform {
    projection: mat4x4<f32>,
}
@group(1) @binding(0)
var<uniform> ui: UiUniform;

@group(0) @binding(0)
var t_diffuse: texture_2d_array<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@group(0) @binding(8)
var t_atlas: texture_2d<f32>;
@group(0) @binding(9)
var s_atlas: sampler;

struct UiVertexInput {
    @location(0) x: u32,
    @location(1) y: u32,
    @location(2) u: f32,
    @location(3) v: f32,
    @location(4) color: u32,
}

struct UiVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: u32,
}

@vertex
fn vs_main(input: UiVertexInput) -> UiVertexOutput {
    var out: UiVertexOutput;
    out.clip_position = ui.projection * vec4<f32>(f32(u32(input.x)), f32(u32(input.y)), 0.0, 1.0);
    out.uv = vec2<f32>(input.u, input.v);
    out.color = input.color;
    return out;
}

const ARRAY_UV_OFFSET: f32 = 2.0;

@fragment
fn fs_main(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let c = in.color;

    // Panneaux colorés : u = -1, v = -1
    if (in.uv.x < 0.0) {
        return vec4<f32>(
            f32((c >> 24u) & 0xFFu) / 255.0,
            f32((c >> 16u) & 0xFFu) / 255.0,
            f32((c >> 8u)  & 0xFFu) / 255.0,
            f32(c & 0xFFu) / 255.0,
        );
    }
    // Arrays : u ∈ [2, 3], v ∈ [0, 1], couche dans color
    if (in.uv.x >= ARRAY_UV_OFFSET) {
        let local_uv = vec2<f32>(in.uv.x - ARRAY_UV_OFFSET, in.uv.y);
        return textureSampleLevel(t_diffuse, s_diffuse, local_uv, c, 0.0);
    }

    // Atlas : u, v ∈ [0, 1], color = teinte
    let tint = vec4<f32>(
        f32((c >> 24u) & 0xFFu) / 255.0,
        f32((c >> 16u) & 0xFFu) / 255.0,
        f32((c >> 8u)  & 0xFFu) / 255.0,
        f32(c & 0xFFu) / 255.0,
    );
    return textureSampleLevel(t_atlas, s_atlas, in.uv, 0.0) * tint;
}
