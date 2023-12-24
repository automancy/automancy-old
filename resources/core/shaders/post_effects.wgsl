struct Uniform {
    _empty: f32,
}

@group(0) @binding(0)
var<uniform> ubo: Uniform;

@group(0) @binding(1)
var frame_texture: texture_2d<f32>;
@group(0) @binding(2)
var frame_sampler: sampler;

@group(0) @binding(3)
var normal_texture: texture_2d<f32>;
@group(0) @binding(4)
var normal_sampler: sampler;

@group(0) @binding(5)
var depth_texture: texture_2d<f32>;
@group(0) @binding(6)
var depth_sampler: sampler;


struct VertexInput {
    @builtin(vertex_index) idx: u32,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(
    in: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;

    let uv = vec2(
         f32((in.idx << 1u) & 2u),
         f32(in.idx & 2u)
     );

    out.pos = vec4(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2(uv.x, 1.0 - uv.y);

    return out;
}

const KERNEL_X = mat3x3<f32>(
    vec3<f32>( 1.0,  0.0, -1.0),
    vec3<f32>( 1.0,  0.0, -1.0),
    vec3<f32>( 1.0,  0.0, -1.0),
);

const KERNEL_Y = mat3x3<f32>(
    vec3<f32>( 1.0,  1.0,  1.0),
    vec3<f32>( 0.0,  0.0,  0.0),
    vec3<f32>(-1.0, -1.0, -1.0),
);

const LUMA = vec3<f32>(0.299, 0.587, 0.114);

fn color_edge(uv: vec2<f32>) -> f32 {
    let texel_size = 1.0 / vec2<f32>(textureDimensions(frame_texture));

    let c  = dot(textureSample(frame_texture, frame_sampler, uv).rgb, LUMA);
    let n  = dot(textureSample(frame_texture, frame_sampler, uv + texel_size * vec2<f32>( 0.0,  1.0)).rgb, LUMA);
    let e  = dot(textureSample(frame_texture, frame_sampler, uv + texel_size * vec2<f32>( 1.0,  0.0)).rgb, LUMA);
    let ne = dot(textureSample(frame_texture, frame_sampler, uv + texel_size * vec2<f32>( 1.0,  1.0)).rgb, LUMA);

    let m = mat3x3(
        vec3( c,  c,  c),
        vec3( c,  c,  e),
        vec3( c,  n, ne),
    );

    let gx = dot(KERNEL_X[0], m[0]) + dot(KERNEL_X[1], m[1]) + dot(KERNEL_X[2], m[2]);
    let gy = dot(KERNEL_Y[0], m[0]) + dot(KERNEL_Y[1], m[1]) + dot(KERNEL_Y[2], m[2]);

    let g = length(vec2(gx, gy));

    return g;
}

fn depth_edge(uv: vec2<f32>) -> f32 {
    let texel_size = 1.0 / vec2<f32>(textureDimensions(depth_texture));

    let c  = textureSample(depth_texture, depth_sampler, uv).r;
    let n  = textureSample(depth_texture, depth_sampler, uv + texel_size * vec2<f32>( 0.0,  1.0)).r;
    let e  = textureSample(depth_texture, depth_sampler, uv + texel_size * vec2<f32>( 1.0,  0.0)).r;
    let ne = textureSample(depth_texture, depth_sampler, uv + texel_size * vec2<f32>( 1.0,  1.0)).r;

    let m = mat3x3(
        vec3( c,  c,  c),
        vec3( c,  c,  e),
        vec3( c,  c, ne),
    );

    let gx = dot(KERNEL_X[0], m[0]) + dot(KERNEL_X[1], m[1]) + dot(KERNEL_X[2], m[2]);
    let gy = dot(KERNEL_Y[0], m[0]) + dot(KERNEL_Y[1], m[1]) + dot(KERNEL_Y[2], m[2]);

    let g = length(vec2(gx, gy));

    return smoothstep(0.4, 1.0, 1.0 - g);
}

fn rgb2hsl(c: vec3<f32>) -> vec3<f32> {
    let K = vec4(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = mix(vec4(c.bg, K.wz), vec4(c.gb, K.xy), step(c.b, c.g));
    let q = mix(vec4(p.xyw, c.r), vec4(c.r, p.yzx), step(p.x, c.r));

    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;

    return vec3(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

fn hsl2rgb(c: vec3<f32>) -> vec3<f32> {
  let K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
  let p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);

  return c.z * mix(K.xxx, clamp(p - K.xxx, vec3(0.0, 0.0, 0.0), vec3(1.0, 1.0, 1.0)), c.y);
}

fn darken(color: vec4<f32>, r: f32) -> vec4<f32> {
    if (r > 0.035) {
        var hsl = rgb2hsl(color.rgb);
        hsl.z *= 0.5;

        return vec4(hsl2rgb(hsl), color.a);
    } else {
        return color;
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(frame_texture, frame_sampler, in.uv);

    let color_edge_c = darken(color, color_edge(in.uv));
    let depth_edge_c = vec4(vec3(depth_edge(in.uv)), 1.0);

    return color_edge_c * depth_edge_c;
}