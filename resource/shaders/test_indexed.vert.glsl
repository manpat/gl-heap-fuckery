out gl_PerVertex {
	vec4 gl_Position;
};

out vec4 v_color;



layout(std140, binding=0) uniform CameraUniforms {
	layout(row_major) mat4 u_projection_view;
};


layout(std430, binding=0) readonly buffer Positions {
	vec4 u_positions[];
};


layout(std430, binding=1) readonly buffer InstanceData {
	vec4 u_colors[];
};


void main() {
	const vec4 position = u_positions[gl_VertexID];
	gl_Position = u_projection_view * (position + vec2(0.0, -0.1 * float(gl_InstanceID)).xxyx);

	v_color = u_colors[gl_InstanceID];
}