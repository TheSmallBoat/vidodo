#version 450

// particles-basic vertex shader
// Animates particle positions using velocity over time.
//
// Uniform block: SceneUniforms (set=0, binding=0)
// Vertex inputs: position (vec3), velocity (vec3)

layout(set = 0, binding = 0) uniform SceneUniforms {
    mat4 view_projection;
    vec4 time_params;    // x=elapsed_sec, y=beat, z=bar, w=tempo
    vec4 color_tint;     // rgba
    vec4 resolution;     // x=width, y=height, z=1/w, w=1/h
};

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 velocity;

layout(location = 0) out vec4 v_color;

void main() {
    float elapsed = time_params.x;

    // Animate position along velocity vector over time
    vec3 animated_pos = position + velocity * elapsed;

    // Fade alpha based on beat phase (pulse on each beat)
    float beat = time_params.y;
    float beat_frac = fract(beat);
    float pulse = 1.0 - beat_frac * 0.5;

    // Pass colour tint to fragment shader with pulse modulation
    v_color = vec4(color_tint.rgb, color_tint.a * pulse);

    gl_Position = view_projection * vec4(animated_pos, 1.0);
    gl_PointSize = 4.0;
}
