struct UiUniform {
    projection: mat4x4<f32>,
}
@group(1) @binding(0)
var<uniform> ui: UiUniform;

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

@fragment
fn fs_main(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let c = in.color;
    let color = vec4<f32>(
        f32((c >> 24u) & 0xFFu) / 255.0,
        f32((c >> 16u) & 0xFFu) / 255.0,
        f32((c >> 8u)  & 0xFFu) / 255.0,
        f32(c & 0xFFu) / 255.0,
    );
    if (in.uv.x >= 0.0 && in.uv.y >= 0.0) {
        return textureSample(t_atlas, s_atlas, in.uv) * color;
    }
    return color;
}
