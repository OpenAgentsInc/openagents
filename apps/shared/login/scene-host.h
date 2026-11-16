// Firm reason: 6 (opaque boundary between the login's two generated Coax libraries and the Apple host).
// The login scene's carrier (scene-host.c): the splash scene (scene.coax) under a component
// host's surface. It is its own translation unit because the scene and the form are separate
// generated libraries.
#ifndef LOGIN_SCENE_HOST_H
#define LOGIN_SCENE_HOST_H
// host: the form's UiHost. resources: the folder holding login-scene-bg.metal and
// login-scene-fx.metal. NULL when the scene cannot start; the form then draws alone.
void *lgs_open(void *host, const char *resources);
// 1 once the scene is drawn under the form (its shaders compile off the main thread).
int lgs_ready(void *scene);
// The form's panel in device pixels (x, y, w, h, corner), and whether the scene is still (reduced motion).
void lgs_form(void *scene, const float form[5], int still);
// 1 while the scene moves on its own.
int lgs_moving(void *scene);
// Frames the scene has computed (a still scene computes one).
unsigned long long lgs_frames(void *scene);
void lgs_close(void *host, void *scene);
#endif
