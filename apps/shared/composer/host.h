// Firm reason: 6 (opaque SDK lifetime boundary; no generated types cross Swift).
// The chat composer's surface (host.c): the compiled composer (surface.coax) on the shared
// component host, as its own Metal view inside a screen the existing runtime still draws. The
// app's native side (apps/shared/native/NativeComposer.swift on iOS) puts gc_view in the dock,
// calls gc_frame from its display link, passes what the chat program's composer element says, and
// sends presses and edits back to that program. Rectangles are in points from the view's top left.
#ifndef COMPOSER_HOST_H
#define COMPOSER_HOST_H
// resources: the folder holding the component shader, fonts and icons. NULL when it cannot open.
void *gc_open(const char *resources);
void *gc_view(void *composer);
// One frame. Call it while gc_moving says 1, and once after anything changes.
void gc_frame(void *composer);
// 1 while the next frame differs with no input: its height, its edge or its action still moves.
int gc_moving(void *composer);
// Called when something changed without the native side's doing (a touch on the surface).
void gc_on_change(void *composer, void (*changed)(void *context), void *context);
// What the chat program's composer says: its field's and its action's ids, the placeholder, the
// model's name, whether a reply runs (Stop), whether the action can be pressed, whether the text is locked.
void gc_set(void *composer, const char *field_id, const char *send_id, const char *placeholder, const char *model, int stop, int enabled, int locked);
// The chips over the bar (the chat home): what each shows and whether it can be pressed. show 0: none.
void gc_chips(void *composer, int show, const char *environment, int environment_on, const char *repository, int repository_on);
// The whole draft: 0 when it is longer than a draft may be. The draft back, its length.
int gc_set_text(void *composer, const char *text);
int gc_text(void *composer, char *out, int cap);
// The platform editor: whether it holds the keyboard, its own text height in points while it does; reduced motion.
void gc_env(void *composer, int focused, float editor_height, int reduced);
// The action asked for, once: 0 none, 1 the action was pressed (send or stop), 2 the text wants the
// keyboard, 3 the environment chip was pressed, 4 the repository chip.
int gc_action(void *composer);
// The height the bar wants in points; the text's committed box (x, y, w, h, the first line's inset,
// a line's height) or 0 before the first frame; the editor's style from the skin (the text's size
// and line in points, then the ink, the placeholder's and the caret's as straight RGBA: 14 floats).
float gc_height(void *composer);
int gc_editor(void *composer, float box[6]);
int gc_style(float style[14]);
// The committed semantic rows: how many, a number that changes when any row does, and row i: its
// box, then its role (0 button, 3 field, 8 text), its state bits (1 disabled, 4 pressed, 64
// focused) and its parent in row[7]; its id, its label and its value. Activating a row by id does what a tap does.
int gc_rows(void *composer);
int gc_rows_stamp(void *composer);
int gc_row(void *composer, int i, float row[7], char *id, int id_cap, char *label, int label_cap, char *value, int value_cap);
int gc_activate(void *composer, const char *id);
// coax drive's operations on the same rows (tree, tap ID, type ID TEXT, counts): the answer's length, or -1.
int gc_drive(void *composer, const char *op, char *out, int cap);
// Frames presented to the screen (a static bar adds none).
unsigned long long gc_presented(void *composer);
void gc_close(void *composer);
#endif
