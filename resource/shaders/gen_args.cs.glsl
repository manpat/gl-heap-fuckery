layout(local_size_x=1, local_size_y=1, local_size_z=1) in;

layout(std430, binding=0) buffer ArgsBuffer {
	uvec3 s_groups;
};

layout(std430, binding=1) buffer ColorBuffer {
	vec4 s_color;
};

void main() {
	s_groups = uvec3(10, 10, 1);
	s_color = vec4(1.0, 0.0, 1.0, 1.0);
}