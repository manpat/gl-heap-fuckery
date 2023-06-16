layout(local_size_x=8, local_size_y=8, local_size_z=1) in;

layout(binding = 0, rgba32f) uniform readonly image2D u_yuv_image;
layout(binding = 1, r11f_g11f_b10f) uniform image2D u_rgb_image;


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


void main() {
	ivec2 sample_position = ivec2(gl_GlobalInvocationID.xy);
	ivec2 image_size = imageSize(u_yuv_image);

	if (any(greaterThanEqual(sample_position, image_size))) {
		return;
	}

	vec3 yuv = imageLoad(u_yuv_image, sample_position).rgb;
	vec3 rgb = yuv_to_rgb(yuv);

	imageStore(u_rgb_image, sample_position, vec4(rgb, 1.0));
}