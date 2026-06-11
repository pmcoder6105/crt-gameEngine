// Elderforge PBR pass.
// TODO: full PBR with IBL; this placeholder does simple n-dot-l shading.

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // TODO: camera view-projection + model matrix uniforms.
    out.clip_position = vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let n_dot_l = max(dot(n, light_dir), 0.0);
    return vec4<f32>(vec3<f32>(0.1 + 0.9 * n_dot_l), 1.0);
}
