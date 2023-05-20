flat in sampler2D v_texture;
in vec4 v_color;
in vec2 v_uv;

out vec4 o_color;

void main() {
	vec4 color = texture(v_texture, v_uv);
	o_color = v_color * color;
}
