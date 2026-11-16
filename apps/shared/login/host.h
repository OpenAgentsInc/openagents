// Firm reason: 6 (opaque SDK lifetime boundary; no generated types cross Swift).
// The native login's surface (host.c): the compiled login screen (screen.coax) on the shared
// component host, over its scene (scene-host.c). The app's native side (apps/shared/native/
// NativeLogin.swift on iOS) puts lg_view in its window, calls lg_frame from its display link,
// and does the effects the screen asks for. Rectangles are in points from the view's top left.
#ifndef LOGIN_HOST_H
#define LOGIN_HOST_H
// resources: the folder holding the component shader, fonts and icons (and the scene's shaders).
// scene 0 opens the form on the plain canvas. NULL when the surface cannot open.
void *lg_open(const char *resources, int scene);
void *lg_view(void *login);
// One frame. Call it while lg_moving says 1, and once after anything changes.
void lg_frame(void *login);
// Called when something changed without the native side's doing (a touch on the surface, a
// driver's operation), so a sleeping display link starts again.
void lg_on_change(void *login, void (*changed)(void *context), void *context);
// 1 while the next frame differs with no input: the scene, the form's motion, glyphs still arriving.
int lg_moving(void *login);
// The window's facts: safe areas and the keyboard's overlap in points, and reduced motion.
void lg_env(void *login, float safe_top, float safe_bottom, float keyboard, int reduced);
// The action the screen asks for, once: 0 none, 1 start the email, 2 verify the code, 3 open the key login.
int lg_action(void *login);
// The answer: ok 1 when the email was sent or the code accepted; message the refusal's words ("" for the screen's own).
void lg_reply(void *login, int action, int ok, const char *message);
// Fields: 1 the email, 2 the code. The focused one (0 none); its value; its box (x, y, w, h and
// the text's inset) or 0 when it is not shown; the whole value after an edit; Return; the keyboard left.
int lg_focus(void *login);
int lg_text(void *login, int field, char *out, int cap);
int lg_field(void *login, int field, float box[5]);
void lg_set_text(void *login, int field, const char *text);
void lg_submit(void *login);
void lg_blur(void *login);
// The platform editor's style from the skin: the text's size and line in points, then the ink,
// the placeholder's and the caret's as straight RGBA (14 floats).
int lg_editor(float style[14]);
// The committed semantic rows: how many, a number that changes when any row does, and row i:
// its box, then its role (components/core/elements.coax: 0 button, 3 field, 4 region, 8 text,
// 9 heading, 13 notice), its state bits (1 disabled, 2 busy, 4 pressed, 64 focused, 1024 live)
// and its parent row (-1 none) in row[7]; its id, its label and its value. Activating a row by
// id does what a tap does.
int lg_rows(void *login);
int lg_rows_stamp(void *login);
int lg_row(void *login, int i, float row[7], char *id, int id_cap, char *label, int label_cap, char *value, int value_cap);
int lg_activate(void *login, const char *id);
// coax drive's operations on the same rows (tree, tap ID, type ID TEXT): the answer's length, or -1.
int lg_drive(void *login, const char *op, char *out, int cap);
// 1 once the code was accepted. 1 while a scene is drawn behind the form.
int lg_done(void *login);
int lg_scene(void *login);
// Frames presented to the screen, and frames the scene computed (a static screen adds none).
unsigned long long lg_presented(void *login);
unsigned long long lg_scene_frames(void *login);
// Frees the surface, the atlas and the scene.
void lg_close(void *login);
#endif
