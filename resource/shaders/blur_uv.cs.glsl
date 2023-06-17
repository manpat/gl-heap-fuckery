layout(local_size_x=8, local_size_y=8) in;

layout(binding = 0) uniform sampler2D u_yuv_src;
layout(binding = 1, rgba16f) uniform writeonly image2D u_yuv_dest;


layout(std140, binding=0) uniform BlurUniforms {
    ivec2 u_direction;
};

void main() {
	ivec2 global_id = ivec2(gl_GlobalInvocationID.xy);
	ivec2 image_size = imageSize(u_yuv_dest);

	if (any(greaterThanEqual(global_id, image_size))) {
		return;
	}

	vec2 resolution = vec2(1.0) / vec2(image_size);
	vec2 centre_uv = (vec2(global_id) + vec2(0.5)) * resolution;

	// hardware filtering blur from https://github.com/Jam3/glsl-fast-gaussian-blur/blob/master/9.glsl
	// I don't think this is technically correct with non-unit step size, but it looks good enough
	vec2 offset_1 = vec2(u_direction) * resolution * 1.3846153846;
	vec2 offset_2 = vec2(u_direction) * resolution * 3.2307692308;

	vec4 texel = texture(u_yuv_src, centre_uv);
	texel.gb *= 0.2270270270;

	texel.gb += texture(u_yuv_src, centre_uv + offset_1).gb * 0.3162162162;
	texel.gb += texture(u_yuv_src, centre_uv - offset_1).gb * 0.3162162162;

	texel.gb += texture(u_yuv_src, centre_uv + offset_2).gb * 0.0702702703;
	texel.gb += texture(u_yuv_src, centre_uv - offset_2).gb * 0.0702702703;

	imageStore(u_yuv_dest, global_id, texel);
}