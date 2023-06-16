layout(local_size_x=16, local_size_y=16, local_size_z=1) in;

layout(binding = 0, r11f_g11f_b10f) uniform image2D u_image;



vec3 rgb_to_yuv(in vec3 rgb){
	float y = 0.299*rgb.r + 0.587*rgb.g + 0.114*rgb.b;
	return vec3(y, 0.493*(rgb.b-y), 0.877*(rgb.r-y));
}

vec3 yuv_to_rgb(in vec3 yuv){
	float y = yuv.x;
	float u = yuv.y;
	float v = yuv.z;
	
	return vec3(
		y + 1.0/0.877*v,
		y - 0.39393*u - 0.58081*v,
		y + 1.0/0.493*u
	);
}


shared vec3 s_samples[2][gl_WorkGroupSize.x*gl_WorkGroupSize.y];

vec3 read_sample(int idx, ivec2 pos) {
	pos = max(min(pos, ivec2(gl_WorkGroupSize.xy) - 1), ivec2(0));
	return s_samples[idx][pos.x + pos.y * gl_WorkGroupSize.x];
}

void write_sample(int idx, ivec2 pos, vec3 value) {
	s_samples[idx][pos.x + pos.y * gl_WorkGroupSize.x] = value;
}


void main() {
	ivec2 global_id = ivec2(gl_GlobalInvocationID.xy);
	ivec2 local_id = ivec2(gl_LocalInvocationID.xy);

	ivec2 image_size = imageSize(u_image);
	ivec2 sample_position = min(image_size - 1, global_id);


	vec4 local_sample = imageLoad(u_image, sample_position);
	vec3 local_yuv = rgb_to_yuv(local_sample.rgb);

	int current_buffer = 0;

	write_sample(current_buffer, local_id, local_yuv);

	barrier();
	groupMemoryBarrier();

	for (int pass = 0; pass < 8; pass++) {
		vec3 sum = vec3(0.0);

		for (int i = -3; i <= 3; i++) {
			sum += read_sample(current_buffer, local_id + ivec2(i, 0));
		}

		current_buffer = 1-current_buffer;

		write_sample(current_buffer, local_id, sum / 7.0);

		barrier();
		groupMemoryBarrier();

		sum = vec3(0.0);

		for (int i = -3; i <= 3; i++) {
			sum += read_sample(current_buffer, local_id + ivec2(0, i));
		}

		current_buffer = 1-current_buffer;

		write_sample(current_buffer, local_id, sum / 7.0);

		barrier();
		groupMemoryBarrier();
	}

	if (any(greaterThanEqual(global_id, image_size))) {
		return;
	}

	vec3 sum = read_sample(current_buffer, local_id);

	local_yuv.bg = sum.bg;
	// local_yuv.r = sum.r;

	// local_yuv.r = 0.5;
	// local_yuv.g = 1.0;
	// local_yuv.b = -0.1;

	vec4 final = vec4(1.0);

	final.rgb = yuv_to_rgb(local_yuv);

	imageStore(u_image, sample_position, final);
}