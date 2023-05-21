out gl_PerVertex {
    vec4 gl_Position;
};

out vec4 v_color;


layout(std140, binding=0) uniform CameraUniforms {
    layout(row_major) mat4 u_projection_view;
};


layout(std140, binding=1) uniform PerDrawUniforms {
    vec4 u_color;
};


layout(std430, binding=0) readonly buffer Positions {
    vec4 u_positions[];
};

layout(std430, binding=1) readonly buffer Indices {
    uint u_indices[];
};


void main() {
    const uint position_index = u_indices[gl_VertexID];
    const vec4 position = u_positions[position_index];
    gl_Position = u_projection_view * position;

    v_color = u_color;
}