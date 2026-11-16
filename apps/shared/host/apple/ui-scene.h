// Firm reason: 6 (opaque Metal boundary for a Coax-emitted scene under the components).
// A scene under the components, in the same Metal pass (components/gpu-components.md, "The first
// screen: login"; GitHub #13645): a full-screen pass, then a pass of the app's own vertices, both
// from shaders Coax emits as Metal (`coax metal FILE --vertex VS --fragment FS`). The host owns
// only the pipelines and the buffers; every number drawn is the app's Coax. It is the Apple
// host's own layer (ui-host-apple.h, ui_layer), so the components are drawn over it.
//
// The emitted shaders' bindings: the uniforms' floats at buffer 0, their count at buffer 29, and
// the vertices at buffer 30 (vertex_floats floats each, four to an attribute). The vertex pass
// blends by the larger of what is there and what it draws, as the web host draws the splash's
// strokes: where two glows meet, their light doesn't double.
#ifndef UI_SCENE_H
#define UI_SCENE_H
#include <stdint.h>
#include "ui-host-apple.h"

typedef struct UiScene UiScene;
typedef struct {
  const char *background;            // the full-screen pass's emitted Metal source (a path)
  const char *background_vs, *background_fs;
  const char *strokes;               // the vertex pass's emitted Metal source (a path)
  const char *strokes_vs, *strokes_fs;
  uint32_t vertex_floats, max_vertices, uniform_floats;
  void *user;
  // One frame: fill the vertices and the uniforms; return the vertex count. w and h are device pixels.
  int (*frame)(void *user, float *vertices, float *uniforms, float w, float h, float dpr, float dt);
  // 1 while the next frame differs from this one with no input (0: a still scene, drawn again only after ui_scene_touch).
  int (*moving)(void *user);
} UiSceneApp;

// Starts the scene under h's components. The shaders compile off the main thread, so the first
// frames are the components alone; ui_scene_ready turns 1 when the scene is drawn. NULL when a
// shader's file is missing; a shader that does not compile leaves the scene not ready for good
// (ui_scene_failed), and the components keep drawing on their own ground.
UiScene *ui_scene_open(UiHost *h, const UiSceneApp *app);
int ui_scene_ready(UiScene *s);
int ui_scene_failed(UiScene *s);
// Something the scene draws from changed (a still scene is asked for its frame again).
void ui_scene_touch(UiScene *s);
// Frames the scene asked the app for since it opened.
uint64_t ui_scene_frames(UiScene *s);
// Takes the scene out from under h's components and frees what it holds.
void ui_scene_close(UiHost *h, UiScene *s);
#endif
