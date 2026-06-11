// Vertex shader
struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(1) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: u32,
    @location(2) tex_layer: u32,
    @location(3) ao: f32,
    @location(4) u: f32,
    @location(5) v: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) ao: f32,
    @location(2) tex_layer: u32,
    @location(3) u: f32,
    @location(4) v: f32,
}

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(model.position, 1.0);
    out.color = vec4<f32>(
        f32(model.color >> 24) / 255.0,
        f32((model.color >> 16) & 0xFFu) / 255.0,
        f32((model.color >> 8) & 0xFFu) / 255.0,
        f32(model.color & 0xFFu) / 255.0,
    );
    out.ao = model.ao;
    out.tex_layer = model.tex_layer;
    out.u = model.u;
    out.v = model.v;
    return out;
}

// Fragment shader

@group(0) @binding(0)
var t_diffuse: texture_2d_array<f32>;
@group(0) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var tex_color: vec4<f32>;
    tex_color = textureSampleLevel(
        t_diffuse,
        s_diffuse,
        vec2<f32>(in.u, in.v),
        in.tex_layer,
        0.0
    );

    // AO: 0.0 = fully occluded (dark), 1.0 = fully lit (bright)
    return vec4<f32>(
        tex_color.r * in.color[0] * in.ao,
        tex_color.g * in.color[1] * in.ao,
        tex_color.b * in.color[2] * in.ao,
        tex_color.a * in.color[3],
    );
}
