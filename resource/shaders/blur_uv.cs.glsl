layout(local_size_x=8, local_size_y=8) in;

layout(binding = 0, rgba16f) uniform readonly image2D u_yuv_src;
layout(binding = 1, rgba16f) uniform writeonly image2D u_yuv_dest;


layout(std140, binding=0) uniform BlurUniforms {
    ivec2 u_direction;
};

// TODO(pat.m): cache texture reads
// shared vec2 s_samples[gl_WorkGroupSize.x*gl_WorkGroupSize.y];

// vec3 read_sample(ivec2 pos) {
// 	pos = max(min(pos, ivec2(gl_WorkGroupSize.xy) - 1), ivec2(0));
// 	return s_samples[pos.x + pos.y * gl_WorkGroupSize.x];
// }

// void write_sample(int idx, ivec2 pos, vec3 value) {
// 	s_samples[idx][pos.x + pos.y * gl_WorkGroupSize.x] = value;
// }

const float weight[5] = float[] (0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);


void main() {
	ivec2 global_id = ivec2(gl_GlobalInvocationID.xy);
	ivec2 image_size = imageSize(u_yuv_src);

	if (any(greaterThanEqual(global_id, image_size))) {
		return;
	}

	vec4 texel = imageLoad(u_yuv_src, global_id);

	texel.gb *= weight[0];

	for (int i = 1; i < 5; i++) {
		ivec2 sample_1 = global_id + u_direction * i;
		ivec2 sample_2 = global_id - u_direction * i;
		texel.gb += imageLoad(u_yuv_src, sample_1).gb * weight[i];
		texel.gb += imageLoad(u_yuv_src, sample_2).gb * weight[i];
	}

	// texel.gb /= 2.0;
	// texel.gb *= 2.0;
	// texel.gb = floor(texel.gb * 4.0) / 4.0;

	imageStore(u_yuv_dest, global_id, texel);
}