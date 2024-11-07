struct WindowUniform {
	size: vec2<f32>,
	scale_factor: f32,
	padding: f32,
}
@group(0) @binding(0)
var<uniform> window: WindowUniform;

struct VertexInput {
	@location(0) position: vec2<f32>,
	@location(1) z_index: f32,
	@location(2) color: vec4<f32>,
	@location(3) border_radius: f32,
	@location(4) rect_pos: vec2<f32>,
	@location(5) rect_size: vec2<f32>,
	@location(6) softness: f32,
}

struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) color: vec4<f32>,
	@location(1) border_radius: f32,
	@location(2) rect_pos: vec2<f32>,
	@location(3) rect_size: vec2<f32>,
	@location(4) softness: f32,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
	var out: VertexOutput;
	
	var signs: vec2<f32> = sign(model.position - model.rect_pos);
	var offset_position = model.position + (signs * model.softness);
	var ndc_position = vec2<f32>(
		(2.0 * offset_position.x / window.size.x) - 1.0,
		1.0 - (2.0 * offset_position.y / window.size.y)
	);
	out.clip_position = vec4<f32>(ndc_position, model.z_index, 1.0);
	out.color = model.color;
	out.rect_pos = model.rect_pos;
	out.rect_size = model.rect_size;
	out.border_radius = model.border_radius;
	out.softness = model.softness;
	return out;
}


fn rect_sdf(point: vec2<f32>, rect_pos: vec2<f32>, rect_size: vec2<f32>, corner_radius: f32) -> f32 {

	var relative_point: vec2<f32> = abs(point - rect_pos);
	
	var shrunk_corner_position = (rect_size / 2.0) - corner_radius;
	var point_to_corner = max(vec2<f32>(0.0, 0.0), relative_point - shrunk_corner_position);

	return length(point_to_corner) - corner_radius;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
	var signed_distance = rect_sdf(in.clip_position.xy, in.rect_pos, in.rect_size, in.border_radius);
	

	if(signed_distance <= 0.0) {
		return in.color;
	} else {
		return vec4<f32>(in.color.x, in.color.y, in.color.z, (1.0 - smoothstep(0.0, in.softness, signed_distance)) * in.color.w); 
	}

	// return vec4<f32>(in.color, smoothstep(175.0, 225.0, in.clip_position.x));
}