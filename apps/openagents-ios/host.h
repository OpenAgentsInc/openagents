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
// The committed semantic rows of the layer that takes presses, for VoiceOver (KitAccess.swift):
// how many, a number that changes when any row does, and row i: the part of its box its clips
// leave (w and h 0 when none shows), its role (components/core/elements.coax EL_*), its state
// bits (EL_S_*), its parent row and its focus place; its id and what a screen reader speaks
// (its whole text). Activating a row by id does what a tap does. The layer (0 the page, 1 what
// covers it) and VoiceOver's scroll of the transcript a page (dir 1 toward the latest, -1 back).
int oc_rows(void *chat);
int oc_rows_stamp(void *chat);
int oc_row(void *chat, int i, float row[8], char *id, int id_cap, char *label, int label_cap);
int oc_activate(void *chat, const char *id);
int oc_layer(void *chat);
int oc_scroll_page(void *chat, int dir);
// The text size the person chose against the default (Dynamic Type: 1 the default). The page draws
// larger, up to its own limit, and the glyph atlas is rasterized again at the new scale. Returns
// the size it draws at; the platform editors take their style (oc_style) times it.
float oc_text_zoom(void *chat, float zoom);
// The shared app core (core-host.h oa_core_open) and the typed login email kept at the last save.
// Call once before the core's launch; oc_step after every call into the core.
void oc_attach(void *chat, void *core, const char *kept);
// The chips' choices per draft as plain text (0 when none), to keep across launches, and back.
// coax drive's cases on the real app (debug builds, -OADrive SOCKET or tcp:PORT with a token): each
// operation runs through the shared driver (apps/shared/app/cases), and while it holds the clock a frame
// gets only the time it grants. w, h are the window in device pixels. 0 if it could not listen.
int oc_drive_start(void *chat, const char *where, const char *token, float w, float h, float dpr);
// The host's own turn after each driven operation (the keyboard a tap asked for, the editor's text), so
// the next operation sees it, as a person's next touch would.
void oc_on_drive_op(void *chat, void (*after)(void *context), void *context);
int oc_env_save(void *chat, char *out, int cap);
void oc_env_restore(void *chat, const char *text);
void oc_step(void *chat);
// The login draft to keep: a number that changes when it does, and its bytes (0: forget it).
int oc_draft_rev(void *chat);
int oc_draft(void *chat, char *out, int cap);
// The id of the field that holds the keyboard ("" none: hide the keyboard): chat.composer,
// login.email or login.code. A login field's box (x, y, w, h), its value, its whole value after an
// edit (0: not taken), Return in it, and the keyboard going away by itself.
int oc_focus(void *chat, char *out, int cap);
int oc_field_box(void *chat, float box[4]);
int oc_field_text(void *chat, char *out, int cap);
int oc_set_field(void *chat, const char *text);
int oc_submit(void *chat);
void oc_blur(void *chat);
// Frames the surface refused to commit (its declared capacity was passed): 0 in a healthy page.
int oc_refused(void *chat);
// Frames presented to the screen (a static page adds none).
unsigned long long oc_presented(void *chat);
// The system controls over the page (GlassChrome.swift): with native on, the page paints neither the
// menu button nor the chips' picker and keeps their rows. The menu button: x, y, side and the page's
// push while the sidebar opens (points), then 1 when the sidebar is open and 1 when it hides. The
// open picker (0 none, 1 where it runs, 2 the repository), a committed row's box by id on either
// layer, and row i's value (what a picker's row says under its name).
void oc_native(void *chat, int on);
int oc_menu(void *chat, float out[6]);
int oc_sheet(void *chat);
int oc_find(void *chat, const char *id, float box[4]);
int oc_row_value(void *chat, int i, char *out, int cap);
void oc_close(void *chat);
// Private SDK functions: caller-owned native buffers only, never the page or drive protocol.
int oi_change(int mode,const char *command,const char *expected,const char *account,const unsigned char *state,int state_n,const char *input,char *out,int cap);
int oi_backup(const unsigned char *raw,int n,const char *expected,char *out,int cap);
int oi_public(const unsigned char *raw,int n,unsigned char *out,int cap);
int oi_identity(const char *account,const char *pubkey,unsigned char *out,int cap);
int oi_http(int method,const char *path,int bytes);
void oc_secret_ready(void *chat,int ready);
int oc_private_mode(void *chat);
void oc_scroll_to(void *chat,float points);
int oc_scroll_info(void *chat,float out[3]);
void oc_native_scroll(void *chat,int on);
int oc_row_visible(void *chat,int row);
#endif

int oi_cancel(const unsigned char *raw,int n,unsigned char *out,int cap);
