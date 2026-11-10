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

// Next issue (next.coax, next-face.c): the button, its panel, and one Claude agent on your login.
void *nxp_new(void);
int32_t nxp_vertex_floats(void);
int32_t nxp_max_vertices(void);
// One frame: polls the agent and draws the button and panel. panel4 gets the panel's box in points
// (all 0 when shut). now is the clock, seconds since the epoch.
int32_t nxp_frame(void *panel, float *vertices, float *panel4, float width, float height, float scale, float dt, int64_t now);
// 1 when the panel took the press or the wheel, so the world doesn't.
int32_t nxp_pointer(void *panel, float x, float y, int32_t phase, float width, float height);
int32_t nxp_wheel(void *panel, float x, float y, float dy, float width, float height);
// 1 while the keys go to the reply; then a key: printable ASCII, 13 Enter, 8 Backspace, 27 Escape.
int32_t nxp_wants_keys(void *panel);
void nxp_key(void *panel, int32_t key);
// What the host is asked to do, once: 0 nothing, 1 run `next-context context`, 2 run it with
// --refresh, 3 run `next-context work N --repo-dir CHECKOUT` for nxp_chosen's issue.
int32_t nxp_take_request(void *panel);
int32_t nxp_chosen(void *panel);
// `next-context work`'s answer: its stdout (the worktree's path, then the issue), its stderr, and whether it exited 0.
void nxp_work(void *panel, const uint8_t *text, int32_t length, const uint8_t *report, int32_t report_length, int32_t ok);
// next-context's answer: its stdout, what it said on stderr, and whether it exited 0.
void nxp_context(void *panel, const uint8_t *text, int32_t length, const uint8_t *report, int32_t report_length, int32_t ok);
// A line waiting to be appended to the AI call ledger, or to the decisions file: its length, 0 when none.
int32_t nxp_take_usage(void *panel, uint8_t *out, int32_t cap);
int32_t nxp_take_decision(void *panel, uint8_t *out, int32_t cap);
// 0 idle, 1 reading the cache, 2 the agent's turn, 3 your turn, 4 failed, 5 stopped.
int32_t nxp_state(void *panel);
// Ends the agent's process group; call when the app quits.
void nxp_close(void *panel);
