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
// Where you stand and what you are doing (x, y, z, facing, act: five floats); 0 before the first frame.
int32_t eg_you(void *state, float *out5);
// The view: the eye, its right, up and forward (four floats each, the forward's last the focal length in pixels); 0 before the first frame.
int32_t eg_camera(void *state, float width, float height, float scale, float *out16);

// Your model (model.coax, model-face.c): the web's player model from Everglade II's pack.
void *egm_new(void);
// The pack's catalog text; 1 when it names every part the model needs.
int32_t egm_catalog(void *model, const uint8_t *text, int32_t length);
int32_t egm_bake_floats(void);
// Every frame of the clips as skinning rows (egm_bake_floats floats); 1 when written.
int32_t egm_bake(void *model, float *out);
int32_t egm_records(void);
int32_t egm_record_floats(void);
// This frame's records (egm_records * egm_record_floats floats) and draws (first index, count,
// record, instances: egm_records * 4 words) from eg_you's five floats. The count of draws.
int32_t egm_frame(void *model, const float *you5, float dt, float *records, uint32_t *draws);
