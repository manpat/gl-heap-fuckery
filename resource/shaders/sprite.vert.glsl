out gl_PerVertex {
    vec4 gl_Position;
};

flat out sampler2D v_texture;
out vec4 v_color;
out vec2 v_uv;


layout(std140, binding=0) uniform CameraUniforms {
    layout(row_major) mat4 u_projection_view;
};


layout(std140, binding=1) uniform SpriteData {
    vec4 u_color;
    sampler2D u_texture;
};



const vec2 c_vertices[] = {
    vec2(-1.0, -1.0),
    vec2(-1.0,  1.0),
    vec2( 1.0,  1.0),

    vec2(-1.0, -1.0),
    vec2( 1.0,  1.0),
    vec2( 1.0, -1.0),
};


void main() {
    const vec2 position = c_vertices[gl_VertexID % 6];
    gl_Position = u_projection_view * vec4(position, 0.0, 1.0);

    v_color = u_color;
    v_uv = position * 0.5 + vec2(0.5);
    v_texture = u_texture;
}