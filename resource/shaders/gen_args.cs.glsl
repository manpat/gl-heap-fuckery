layout(local_size_x=1, local_size_y=1, local_size_z=1) in;

layout(std430, binding=0) buffer ArgsBuffer {
	uvec3 s_groups;
};


void main() {
	s_groups = uvec3(1, 1, 1);
}