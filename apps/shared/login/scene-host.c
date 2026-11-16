// Firm reason: 6 (carries a Coax scene into the Apple host's Metal layer; holds no behaviour).
// See scene-host.h. The scene's vertices, uniforms and clock are scene.coax's; its shaders are
// scene-shader.coax emitted as Metal.
#include "login-scene.h"
#include "scene-host.h"
#include "../host/apple/ui-scene.h"
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
  coax_t_LoginScene state;
  UiScene *scene;
  float form[5];
  int still;
  uint32_t max_vertices, vertex_floats, uniform_floats;
} Lgs;

static int lgs_frame(void *user, float *vertices, float *uniforms, float w, float h, float dpr, float dt) {
  Lgs *l = user;
  return (int)login_scene_frame((coax_v_t_LoginScene){&l->state, 1}, (coax_v_f32){vertices, (int64_t)l->max_vertices * l->vertex_floats},
                                (coax_v_f32){uniforms, l->uniform_floats}, w, h, dpr, dt, l->still != 0, l->form[0], l->form[1], l->form[2], l->form[3], l->form[4]);
}
static int lgs_is_moving(void *user) { return !((Lgs *)user)->still; }

void *lgs_open(void *host, const char *resources) {
  if (!host || !resources) return NULL;
  Lgs *l = calloc(1, sizeof *l);
  if (!l) return NULL;
  char background[PATH_MAX + 32], strokes[PATH_MAX + 32];
  snprintf(background, sizeof background, "%s/login-scene-bg.metal", resources);
  snprintf(strokes, sizeof strokes, "%s/login-scene-fx.metal", resources);
  l->max_vertices = (uint32_t)login_scene_max_vertices(); l->vertex_floats = (uint32_t)login_scene_vertex_floats(); l->uniform_floats = (uint32_t)login_scene_uniform_floats();
  UiSceneApp app = {.background = background, .background_vs = "login_bg_vs", .background_fs = "login_bg_fs",
                    .strokes = strokes, .strokes_vs = "login_fx_vs", .strokes_fs = "login_fx_fs",
                    .vertex_floats = l->vertex_floats, .max_vertices = l->max_vertices, .uniform_floats = l->uniform_floats,
                    .user = l, .frame = lgs_frame, .moving = lgs_is_moving};
  l->scene = ui_scene_open(host, &app);
  if (!l->scene) { free(l); return NULL; }
  return l;
}
int lgs_ready(void *scene) { return scene && ui_scene_ready(((Lgs *)scene)->scene); }
void lgs_form(void *scene, const float form[5], int still) {
  Lgs *l = scene;
  if (!l) return;
  if (!memcmp(l->form, form, sizeof l->form) && l->still == (still != 0)) return;
  memcpy(l->form, form, sizeof l->form); l->still = still != 0;
  ui_scene_touch(l->scene);
}
int lgs_moving(void *scene) { Lgs *l = scene; return l && ui_scene_ready(l->scene) && !l->still; }
unsigned long long lgs_frames(void *scene) { return scene ? ui_scene_frames(((Lgs *)scene)->scene) : 0; }
void lgs_close(void *host, void *scene) {
  Lgs *l = scene;
  if (!l) return;
  ui_scene_close(host, l->scene);
  free(l);
}
