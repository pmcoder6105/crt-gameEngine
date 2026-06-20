// Forward unlit-but-shaded pass: transforms geometry by a per-object model
// matrix and a shared camera view-projection, then applies a single
// hard-coded directional light so depth and orientation read clearly. The
// real PBR pass supersedes this; it exists to put scene geometry on screen.

struct Globals {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> globals: Globals;

// Per-object model matrix, bound with a dynamic offset (one slot per draw).
struct ModelData {
    model: mat4x4<f32>,
};
@group(1) @binding(0) var<uniform> object: ModelData;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let world = object.model * vec4<f32>(in.position, 1.0);
    out.clip_position = globals.view_proj * world;
    // Uniform scale only, so the model matrix transforms normals directly.
    out.world_normal = (object.model * vec4<f32>(in.normal, 0.0)).xyz;
    return out;
}

@fragment
fn fs_main(in: VsOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    // Two-sided shading: culling is off engine-wide, so thin surfaces (cloth)
    // rasterize both faces. Flip the normal toward the viewer on back faces so a
    // flag is lit and tinted on whichever side is showing, not black on one.
    var n = normalize(in.world_normal);
    if (!front_facing) {
        n = -n;
    }
    let light = normalize(vec3<f32>(0.3, 0.9, 0.35));
    let diffuse = clamp(dot(n, light), 0.0, 1.0);
    let shade = 0.35 + 0.65 * diffuse;
    // Tint by orientation so faces are distinguishable as objects tumble.
    let base = 0.5 + 0.5 * n;
    return vec4<f32>(base * shade, 1.0);
}
