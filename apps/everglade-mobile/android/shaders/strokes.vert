#version 450
// Everglade for Android: what is drawn over the place and your model. The same strokes and text
// as apps/everglade-desktop/Workshop.metal (fx_vs); keep the two in step.
layout(push_constant) uniform U { vec2 size; float time; float pad; } u;
layout(location = 0) in vec4 a;
layout(location = 1) in vec4 b;
layout(location = 2) in vec4 c;
layout(location = 0) out vec2 place;
layout(location = 1) out vec4 shape;
layout(location = 2) out vec4 tint;
void main() {
  gl_Position = vec4(a.x / u.size.x * 2.0 - 1.0, 1.0 - a.y / u.size.y * 2.0, 0.0, 1.0);
  place = a.zw; shape = b; tint = c;
}
