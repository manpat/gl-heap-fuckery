layout(local_size_x=8, local_size_y=8, local_size_z=1) in;

layout(binding = 0, r11f_g11f_b10f) uniform readonly image2D u_rgb_image;
layout(binding = 1, rgba16f) uniform image2D u_yuv_image;



vec3 rgb_to_yuv(in vec3 rgb){
	float y = 0.299*rgb.r + 0.587*rgb.g + 0.114*rgb.b;
	return vec3(y, 0.493*(rgb.b-y), 0.877*(rgb.r-y));
}



void main() {
	ivec2 sample_position = ivec2(gl_GlobalInvocationID.xy);
	ivec2 image_size = imageSize(u_rgb_image);

	if (any(greaterThanEqual(sample_position, image_size))) {
		return;
	}

	vec3 rgb = imageLoad(u_rgb_image, sample_position).rgb;
	vec3 yuv = rgb_to_yuv(rgb);

	imageStore(u_yuv_image, sample_position, vec4(yuv, 1.0));
}