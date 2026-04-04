#version 450

// particles-basic fragment shader
// Applies colour + alpha blending from the vertex stage.

layout(location = 0) in vec4 v_color;

layout(location = 0) out vec4 frag_color;

void main() {
    // Alpha-blended output: pre-multiplied alpha
    frag_color = vec4(v_color.rgb * v_color.a, v_color.a);
}
