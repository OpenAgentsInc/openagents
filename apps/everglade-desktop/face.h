// Everglade for desktop: the host's C face over workshop.coax. A host allocates
// the state and the vertex buffer, and calls these each frame and on input.
#pragma once
#include <stdint.h>

void *eg_new(void);
int32_t eg_vertex_floats(void);
int32_t eg_max_vertices(void);
// Draws one frame into vertices; width and height in points, scale in pixels a point. The vertex count.
int32_t eg_frame(void *state, float *vertices, float width, float height, float scale, float dt);
// phase 0 down, 1 moved, 2 up; x and y in points from the top left.
void eg_pointer(void *state, float x, float y, int32_t phase);
void eg_wheel(void *state, float dy);
// A key held or let go, in the web realms' codes: a letter's ASCII, 32 Space, -13 Shift,
// -4 -5 -6 -7 the arrows left, right, up, down, -1 the window lost the keys. 1 when used.
int32_t eg_key(void *state, int32_t key, int32_t down);
