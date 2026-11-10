// Everglade's apps: the host's C face over scenes.coax (the worlds: the list you open on, the
// Workshop, the Archery Range). A host allocates the state and the vertex buffer, and calls these
// each frame and on input; the Coax sends each to the world that is open.
#pragma once
#include <stdbool.h>
#include <stdint.h>

void *eg_new(void);
int32_t eg_vertex_floats(void);
int32_t eg_max_vertices(void);
// Draws one frame into vertices; width and height in points, scale in pixels a point. The vertex count.
int32_t eg_frame(void *state, float *vertices, float width, float height, float scale, float dt);
// phase 0 down, 1 moved, 2 up; x and y in points from the top left.
void eg_pointer(void *state, float x, float y, int32_t phase);
void eg_wheel(void *state, float dy);
// A touch stick: how far it is pushed right and forward, each -1 to 1 (0, 0 when let go).
void eg_pad(void *state, float x, float y);
// The points the screen's edges keep clear (a notch, a home bar), so the text stays out from under them.
void eg_safe(void *state, float left, float top, float bottom);
// The phone's turn as camera look (a gyro). A host that can read it offers it once: the scene then
// draws a Gyro switch, on to begin with. eg_turns is 1 while it is on; eg_turn gives the turn since
// the last call, in radians, signed as a drag (yaw right negative, pitch up positive). eg_tap is 1
// when a tap at that point was on the switch, which it flips, so the host doesn't jump.
void eg_turn_offer(void *state);
int32_t eg_turns(void *state);
void eg_turn(void *state, float yaw, float pitch);
int32_t eg_tap(void *state, float x, float y);
// Which world is open: 0 the list, 1 Everglade (the main flow), 2 the Workshop, 3 the Archery
// Range, 4 Experimental. eg_go opens one from outside (a launch argument).
int32_t eg_scene(void *state);
void eg_go(void *state, int32_t world);
// A host that runs a realm itself says so (world 1: Everglade, the web's everglade2 realm, through
// aethyr/host/apple/page-host.h). While that world is open the host draws the realm and sends it
// the keys and presses; eg_frame then draws only the Worlds button, and eg_on_worlds is 1 where a
// press is that button's (give it to eg_pointer, not the realm).
void eg_runs(void *state, int32_t world, int32_t runs);
int32_t eg_on_worlds(void *state, float x, float y);
// The place to draw behind it: 1 the temple (plc_), 2 the range (rpl_).
int32_t eg_place(void *state);
// 1 when a touch that begins at this point is the list's or a button's, so it is a press
// (eg_pointer), never the stick that moves you.
int32_t eg_claims(void *state, float x, float y);
// A key held or let go, in the web realms' codes: a letter's or digit's ASCII, 32 Space, 27 Escape
// (back to the list), -13 Shift, -4 -5 -6 -7 the arrows left, right, up, down, -1 the window lost
// the keys. 1 when used.
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
// Coding turns that have ended since the app opened.
int32_t nxp_runs(void *panel);
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
// The Golems board (golem-board.coax): every agent on this Mac. nxp_frame's panel4 holds 8 floats:
// Next issue's box, then the board's. When nxp_golems_due says so, run everglade-golems snapshot
// --now NOW and hand its stdout to nxp_golems. nxp_golems_open opens the board (--golems).
int32_t nxp_golems_due(void *panel, int64_t now);
void nxp_golems(void *panel, const uint8_t *text, int32_t length, int64_t now);
void nxp_golems_open(void *panel);

// The place (everglade/workshop/temple.coax, place-face.c): the temple's parts as one mesh, the
// records that stand them, and the draws, built once from Aethyr's architecture kit.
void *plc_new(void);            // builds it; NULL when it did not fit
int32_t plc_vertex_floats(void);
int32_t plc_record_floats(void);
int32_t plc_vertices(void *place);
int32_t plc_records(void *place);
int32_t plc_draws(void *place);
// A group's first draw and how many: 0 what stands on the floor, 1 the floor, 2 what is only light.
int32_t plc_group_first(void *place, int32_t group);
int32_t plc_group_count(void *place, int32_t group);
int32_t plc_copy_mesh(void *place, float *out, int32_t floats);
int32_t plc_copy_records(void *place, float *out, int32_t floats);
int32_t plc_copy_draws(void *place, uint32_t *out, int32_t words);

// The Archery Range's place (everglade/range/place.coax, range-place-face.c): the same calls and
// the same groups as the temple's, so a host draws either with one piece of code.
void *rpl_new(void);
int32_t rpl_vertex_floats(void);
int32_t rpl_record_floats(void);
int32_t rpl_vertices(void *place);
int32_t rpl_records(void *place);
int32_t rpl_draws(void *place);
int32_t rpl_group_first(void *place, int32_t group);
int32_t rpl_group_count(void *place, int32_t group);
int32_t rpl_copy_mesh(void *place, float *out, int32_t floats);
int32_t rpl_copy_records(void *place, float *out, int32_t floats);
int32_t rpl_copy_draws(void *place, uint32_t *out, int32_t words);

// A Mac's wheel travel as the host contract's (aethyr/host/appkit-input.coax): down positive, a
// trackpad's pixels as they are, a wheel's tick forty.
float hmi_appkit_wheel(float delta, bool precise);
