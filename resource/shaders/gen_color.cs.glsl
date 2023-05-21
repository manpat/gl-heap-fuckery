layout(local_size_x=1, local_size_y=1, local_size_z=1) in;

layout(std430, binding=0) buffer ColorBuffer {
	vec4 s_color;
};


void main() {
	vec4 color = s_color;

	for (int i = 0; i < 2; i++) {
		color = mix(color, vec4(0.2, 0.5, 1.0, 1.0), 0.01);
	}

	s_color = color;
}