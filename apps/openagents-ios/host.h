// Firm reason: 6 (opaque SDK lifetime boundary; no generated types cross Swift).
// The chat page's surface (host.c): the compiled chat page (chat.coax) on the shared component
// host, in one Metal view. App.swift puts oc_view in its window, calls oc_frame from its display
// link and does the effects the page asks for. Rectangles are in points from the view's top left.
#ifndef OPENAGENTS_CHAT_HOST_H
#define OPENAGENTS_CHAT_HOST_H
// resources: the folder holding the component shader, fonts and icons. NULL when it cannot open.
void *oc_open(const char *resources);
void *oc_view(void *chat);
// One frame. Call it while oc_moving says 1, and once after anything changes.
void oc_frame(void *chat);
int oc_moving(void *chat);
// Called when something changed without the native side's doing (a touch on the surface).
void oc_on_change(void *chat, void (*changed)(void *context), void *context);
// The window's facts in points, the platform editor (whether it holds the keyboard, its own text
// height) and reduced motion.
void oc_env(void *chat, float safe_top, float safe_bottom, float keyboard, int focused, float editor_height, int reduced);
// The action asked for, once: 0 none, 1 the composer's text wants the keyboard.
int oc_action(void *chat);
// The draft: the whole text after an edit (0 when too long), and the page's own.
int oc_set_text(void *chat, const char *text);
int oc_text(void *chat, char *out, int cap);
// The composer text's committed box (x, y, w, h, the first line's inset, a line's height), and the
// editor's style from the skin (14 floats: the text's size and line, then the ink, the
// placeholder's and the caret's as straight RGBA).
int oc_editor(void *chat, float box[6]);
int oc_style(float style[14]);
// The committed semantic rows: how many, a number that changes when any row does, and row i: its
// box, then its role (0 button, 3 field, 8 text, 9 heading, 13 notice), its state bits (1 disabled,
// 64 focused, 1024 live) and its parent in row[7]; its id and its label. Activating a row by id
// does what a tap does.
int oc_rows(void *chat);
int oc_rows_stamp(void *chat);
int oc_row(void *chat, int i, float row[7], char *id, int id_cap, char *label, int label_cap);
int oc_activate(void *chat, const char *id);
// Frames the surface refused to commit (its declared capacity was passed): 0 in a healthy page.
int oc_refused(void *chat);
// Frames presented to the screen (a static page adds none).
unsigned long long oc_presented(void *chat);
void oc_close(void *chat);
#endif
