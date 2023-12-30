struct Uniform {
    light_color: vec4<f32>,
    world_matrix: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> ubo: Uniform;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
}

struct InstanceInput {
    @location(3) color_offset: vec4<f32>,
    @location(4) alpha: f32,
    @location(5) light_pos: vec3<f32>,

    @location(6) model_matrix_0: vec4<f32>,
    @location(7) model_matrix_1: vec4<f32>,
    @location(8) model_matrix_2: vec4<f32>,
    @location(9) model_matrix_3: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) light_pos: vec3<f32>,
    @location(3) model_pos: vec3<f32>,
}

@vertex
fn vs_main(
    in: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    let model_matrix = mat4x4(
       instance.model_matrix_0,
       instance.model_matrix_1,
       instance.model_matrix_2,
       instance.model_matrix_3,
   );
    let model_pos = model_matrix * vec4(in.pos, 1.0);
    out.model_pos = model_pos.xyz / model_pos.w;
    out.pos = ubo.world_matrix * model_pos;

   let normal_matrix = mat3x3(
      instance.model_matrix_0.xyz,
      instance.model_matrix_1.xyz,
      instance.model_matrix_2.xyz,
   );
    out.normal = normalize(normal_matrix * in.normal);

    out.color = vec4(mix(instance.color_offset.rgb, in.color.rgb, in.color.a - instance.color_offset.a), instance.alpha * in.color.a);
    out.light_pos = instance.light_pos;

    return out;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) depth: f32,
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
	let nn = normalize(in.normal);
    let light_dir = normalize(in.model_pos - in.light_pos);

    let diff = max(dot(-light_dir, nn), 0.0);

    var out: FragmentOutput;

    out.color = vec4(in.color.rgb * ubo.light_color.rgb * clamp(diff, 0.25, ubo.light_color.a), in.color.a);
    out.normal = vec4(in.normal, 0.0);
    out.depth = in.model_pos.z;

    return out;
}