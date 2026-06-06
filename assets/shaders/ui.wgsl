struct UiUniform {
    projection: mat4x4<f32>,
}
@group(0) @binding(0)
var<uniform> ui: UiUniform;

@group(0) @binding(1)
var t_atlas: texture_2d<f32>;
@group(0) @binding(2)
var s_atlas: sampler;

struct UiVertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct UiVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = ui.projection * vec4<f32>(input.position, 0.0, 1.0);
    out.uv = input.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_atlas, s_atlas, in.uv);
}
