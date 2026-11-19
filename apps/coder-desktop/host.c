// Firm reason: 6 (native GPU carrier for the compiled page; state, layout and paint are Coax).
// Coder's page on the shared carrier (host.h). carrier.c includes it after the component
// carrier (components/gallery/linux/main.c), so it shares that file's App and atlas code and the
// one generated library. It holds no behaviour: frames, clicks and lines go to page.coax.
#include "host.h"

typedef struct {
  App app;
  coax_t_CoderPage *s;
  int edge;
  int focused, reduced;
  float editor_height;
  float screen;   // the screen's own scale
  int settle;     // frames still owed after a change (glyphs asked for in one frame are drawn in the next)
  void (*changed)(void *context); void *context;
  int headless;   // a page with no window: a chat whose reply runs out of view
} CoderHost;

static coax_v_t_CoderPage cd_state(CoderHost *c) { return (coax_v_t_CoderPage){c->s, 1}; }
static coax_string cd_string(const char *text) { return (coax_string){text ? text : "", text ? (int64_t)strlen(text) : 0}; }
static void cd_changed(CoderHost *c) { c->settle = 3; }
static void cd_woke(CoderHost *c) { cd_changed(c); if (c->changed) c->changed(c->context); }

static int cd_on_frame(void *user, float *vertices, float w, float h, float dpr, float dt) {
  CoderHost *c = user;
  coder_page_env(cd_state(c), c->focused != 0, c->editor_height, c->reduced != 0);
  int n = (int)coder_page_frame(cd_state(c), (coax_v_f32){vertices, (int64_t)c->app.max_vertices * coaxial_components_vertex_floats()}, w, h, dpr, dt);
  if (atlas_serve(&c->app)) cd_changed(c);
  if (c->settle > 0) c->settle--;
  return n < 0 ? 0 : n;
}
static float cd_icon_size(int i) { return coder_page_icon_size(i); }
static void cd_on_pointer(void *user, float x, float y, int phase, int clicks, int mods) {
  (void)clicks; (void)mods;
  CoderHost *c = user;
  coder_page_pointer(cd_state(c), x, y, phase);
  // over a sidebar's edge, and while one is dragged, the pointer is the resize arrows
  int edge = coder_page_edge(cd_state(c)) ? 3 : 0;
  if (edge != c->edge) { c->edge = edge; ui_cursor(c->app.host, edge); }
  cd_woke(c);
}
static void cd_on_wheel(void *user, float dy) { CoderHost *c = user; coder_page_wheel(cd_state(c), dy); cd_woke(c); }

void *cd_open(const char *resources) {
  CoderHost *c = calloc(1, sizeof *c);
  if (!c) return NULL;
  App *a = &c->app;
  snprintf(a->resources, sizeof a->resources, "%s", resources);
  char shader[PATH_MAX + 32]; snprintf(shader, sizeof shader, "%s/components.metal", resources);
  c->s = calloc(1, sizeof *c->s);
  if (!c->s) { free(c); return NULL; }
  a->icon_size = cd_icon_size;
  a->atlas = c->s->atlas.e; a->max_vertices = (uint32_t)coder_page_max_vertices();
  UiApp app = {.title = "Coder", .width = 900, .height = 640, .embedded = 1, .max_vertices = a->max_vertices, .shader = shader, .user = c,
               .frame = cd_on_frame, .pointer = cd_on_pointer, .wheel = cd_on_wheel};
  a->host = ui_open(&app, 0);
  if (!a->host) { free(c->s); free(c); return NULL; }
  // the atlas at the scale the page draws its units at
  a->dpr = ui_dpr(a->host); if (a->dpr < 1) a->dpr = 1;
  c->screen = a->dpr;
  a->dpr *= coder_page_scale(cd_state(c));
  if (!atlas_setup(a)) { gallery_free(a); free(c->s); free(c); return NULL; }
  cd_changed(c);
  return c;
}
// The text size (a row of the page's sizes: 0 to 7, 2 the usual). The fonts and icons are drawn
// again at the new scale, so text stays sharp. Returns the row it is at.
static int cd_rescale(CoderHost *c, int row) {
  App *a = &c->app;
  float want = c->screen * coder_page_scale(cd_state(c));
  if (want == a->dpr) return row;
  for (int i = 0; i < 4; i++) { free(a->font_bytes[i]); a->font_bytes[i] = NULL; }
  for (int i = 0; i < 64; i++) { free(a->svg[i]); a->svg[i] = NULL; }
  free(a->mono); free(a->color); free(a->work); a->mono = NULL; a->color = NULL; a->work = NULL;
  memset(a->atlas, 0, sizeof *a->atlas); memset(&a->plan, 0, sizeof a->plan); memset(&a->fonts, 0, sizeof a->fonts);
  a->dpr = want;
  if (!atlas_setup(a)) fprintf(stderr, "Coder: the fonts could not be drawn at the new size\n");
  cd_changed(c);
  return row;
}
int cd_zoom(void *page, int by) { CoderHost *c = page; return c ? cd_rescale(c, (int)coder_page_zoom(cd_state(c), by)) : 2; }
int cd_zoom_set(void *page, int row) { CoderHost *c = page; return c ? cd_rescale(c, (int)coder_page_zoom_set(cd_state(c), row)) : 2; }
void *cd_view(void *page) { return page ? ui_view(((CoderHost *)page)->app.host) : NULL; }
void cd_frame(void *page) { if (page) ui_frame(((CoderHost *)page)->app.host); }
int cd_moving(void *page) { CoderHost *c = page; return c && (c->settle > 0 || coder_page_moving(cd_state(c))); }
void cd_on_change(void *page, void (*changed)(void *context), void *context) { CoderHost *c = page; if (c) { c->changed = changed; c->context = context; } }
void cd_env(void *page, int focused, float editor_height, int reduced) {
  CoderHost *c = page;
  if (!c) return;
  if (c->focused != (focused != 0) || c->editor_height != editor_height || c->reduced != (reduced != 0)) cd_changed(c);
  c->focused = focused != 0; c->editor_height = editor_height; c->reduced = reduced != 0;
}
int cd_set_text(void *page, const char *text) { CoderHost *c = page; if (!c) return 0; cd_changed(c); return coder_page_set_text(cd_state(c), cd_string(text)); }
int cd_new(void *page) { CoderHost *c = page; if (!c) return 0; cd_changed(c); return coder_page_new(cd_state(c)); }
int cd_submit(void *page) { CoderHost *c = page; if (!c) return 0; cd_changed(c); return coder_page_submit(cd_state(c)); }
int cd_action(void *page) { CoderHost *c = page; return c ? (int)coder_page_action(cd_state(c)) : 0; }
int cd_task(void *page, const char *session, const char *cwd, char *out, int cap) {
  CoderHost *c = page;
  return c && cap > 0 ? (int)coder_page_task(cd_state(c), cd_string(session), cd_string(cwd), (coax_v_u8){(uint8_t *)out, cap}) : 0;
}
void cd_skills(void *page, const char *note) { CoderHost *c = page; if (c) coder_page_skills(cd_state(c), cd_string(note)); }
void cd_folder(void *page, const char *shown) { CoderHost *c = page; if (c) { cd_changed(c); coder_page_folder(cd_state(c), cd_string(shown)); } }
int cd_answer_line(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 0 ? (int)coder_page_answer_line(cd_state(c), (coax_v_u8){(uint8_t *)out, cap}) : 0; }
int cd_agent_next(void *page) { CoderHost *c = page; return c ? (int)coder_page_agent_next(cd_state(c)) : -1; }
int cd_agent_task(void *page, int agent, const char *session, const char *cwd, char *out, int cap) {
  CoderHost *c = page;
  return c && cap > 0 ? (int)coder_page_agent_task(cd_state(c), agent, cd_string(session), cd_string(cwd), (coax_v_u8){(uint8_t *)out, cap}) : 0;
}
void cd_agent_line(void *page, int agent, const char *line) { CoderHost *c = page; if (!c) return; coder_page_agent_line(cd_state(c), agent, cd_string(line)); cd_changed(c); }
void cd_agent_failed(void *page, int agent, const char *why) { CoderHost *c = page; if (!c) return; coder_page_agent_failed(cd_state(c), agent, cd_string(why)); cd_changed(c); }
int cd_agent_answer_line(void *page, int agent, char *out, int cap) { CoderHost *c = page; return c && cap > 0 ? (int)coder_page_agent_answer_line(cd_state(c), agent, (coax_v_u8){(uint8_t *)out, cap}) : 0; }
void cd_claude(void *page, int on) { CoderHost *c = page; if (c) coder_page_set_claude(cd_state(c), on != 0); }
// The sidebars' widths as dragged: read (1 once after a drag, when they are to be kept) and set.
int cd_sides(void *page, float out[2]) { CoderHost *c = page; return c && coder_page_sides(cd_state(c), (coax_v_f32){out, 2}); }
void cd_sides_set(void *page, float left, float right) { CoderHost *c = page; if (c) { coder_page_sides_set(cd_state(c), left, right); cd_woke(c); } }
int cd_pane(void *page, int n) { CoderHost *c = page; return c && coder_page_pane(cd_state(c), n); }
int cd_agent_claude(void *page, int agent) { CoderHost *c = page; return c && coder_page_agent_claude(cd_state(c), agent); }
int cd_agent_running(void *page, int agent) { CoderHost *c = page; return c && coder_page_agent_running(cd_state(c), agent); }
// Kept chats (page.coax): the list, the chat that is shown, and a chat's text each way.
static int cd_text_out(int n, char *out, int cap) { if (n < 0 || n >= cap) n = 0; out[n] = 0; return n; }
void cd_chats(void *page, const char *list) { CoderHost *c = page; if (c) { coder_page_chats(cd_state(c), cd_string(list)); cd_changed(c); } }
void cd_current(void *page, const char *id) { CoderHost *c = page; if (c) { coder_page_current(cd_state(c), cd_string(id)); cd_changed(c); } }
void cd_foreman(void *page, int busy, int tier) { CoderHost *c = page; if (c) { coder_page_foreman(cd_state(c), busy != 0, tier); cd_changed(c); } }
int cd_open_id(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_open_id(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_pin_move(void *page, char *id_out, int cap, int *to_out) {
  CoderHost *c = page;
  if (!c || cap < 2 || !to_out) return 0;
  int n = cd_text_out((int)coder_page_pin_move_id(cd_state(c), (coax_v_u8){(uint8_t *)id_out, cap - 1}), id_out, cap);
  *to_out = (int)coder_page_pin_move_to(cd_state(c));
  return n;
}
int cd_chat_at(void *page, float x, float y, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_chat_at(cd_state(c), x, y, (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_foreman_at(void *page, float x, float y) { CoderHost *c = page; return c ? (int)coder_page_foreman_at(cd_state(c), x, y) : 0; }
int cd_title(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_title(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_save_due(void *page) { CoderHost *c = page; return c && coder_page_save_due(cd_state(c)); }
int cd_save(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 1 ? cd_text_out((int)coder_page_save(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1}), out, cap) : 0; }
int cd_load(void *page, const char *text) { CoderHost *c = page; if (!c) return 0; cd_changed(c); return coder_page_load(cd_state(c), cd_string(text)); }
int cd_recall_line(void *page, char *out, int cap) { CoderHost *c = page; return c && cap > 0 ? (int)coder_page_recall_line(cd_state(c), (coax_v_u8){(uint8_t *)out, cap}) : 0; }
int cd_stop_line(char *out, int cap) { return cap > 0 ? (int)coder_page_stop_line((coax_v_u8){(uint8_t *)out, cap}) : 0; }
int cd_forget_line(char *out, int cap) { return cap > 0 ? (int)coder_page_forget_line((coax_v_u8){(uint8_t *)out, cap}) : 0; }
void cd_line(void *page, const char *line) { CoderHost *c = page; if (!c) return; coder_page_line(cd_state(c), cd_string(line)); cd_changed(c); }
void cd_failed(void *page, const char *why) { CoderHost *c = page; if (!c) return; coder_page_failed(cd_state(c), cd_string(why)); cd_changed(c); }
int cd_running(void *page) { CoderHost *c = page; return c && coder_page_running(cd_state(c)); }
int64_t cd_cost_usd_micros(void *page) { CoderHost *c = page; return c ? coder_page_cost_usd_micros(cd_state(c)) : 0; }
int64_t cd_budget_usd_micros(void *page) { CoderHost *c = page; return c ? coder_page_budget_usd_micros(cd_state(c)) : 50000000; }
int cd_set_budget_usd_micros(void *page, int64_t micros) { CoderHost *c = page; return c ? (int)coder_page_set_budget_usd_micros(cd_state(c), micros) : 0; }
int cd_budget_exceeded(void *page) { CoderHost *c = page; return c ? (int)coder_page_budget_exceeded(cd_state(c)) : 0; }
int cd_budget_approve(void *page) { CoderHost *c = page; return c ? (int)coder_page_budget_approve(cd_state(c)) : 0; }
int cd_goal_status(void *page) { CoderHost *c = page; return c ? (int)coder_page_goal_status(cd_state(c)) : 0; }
int cd_goal_time(void *page) { CoderHost *c = page; return c ? (int)coder_page_goal_time(cd_state(c)) : 0; }
int cd_goal_objective(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || cap < 1) return 0;
  int n = (int)coder_page_goal_objective(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n] = 0;
  return n;
}
int cd_goal_set(void *page, const char *objective) { CoderHost *c = page; if (!c) return 0; int ok = (int)coder_page_goal_set(cd_state(c), cd_string(objective)); cd_changed(c); return ok; }
int cd_goal_set_status(void *page, int status) { CoderHost *c = page; if (!c) return 0; int ok = (int)coder_page_goal_set_status(cd_state(c), status); cd_changed(c); return ok; }
int cd_goal_clear(void *page) { CoderHost *c = page; if (!c) return 0; int ok = (int)coder_page_goal_clear(cd_state(c)); cd_changed(c); return ok; }
int cd_copied(void *page, char *out, int cap) {
  CoderHost *c = page;
  if (!c || cap < 1) return 0;
  int n = (int)coder_page_copied(cd_state(c), (coax_v_u8){(uint8_t *)out, cap - 1});
  out[n] = 0; cd_changed(c);
  return n;
}
int cd_editor(void *page, float box[6]) { CoderHost *c = page; return c && coder_page_editor(cd_state(c), (coax_v_f32){box, 6}); }
int cd_style(void *page, float style[14]) { CoderHost *c = page; return c ? coder_page_style(cd_state(c), (coax_v_f32){style, 14}) : 0; }
// A page with no window, for a chat whose reply goes on out of view: it reads its workers' lines
// and keeps its text as the window's page does. cd_close frees it.
void *cd_headless(void) {
  CoderHost *c = calloc(1, sizeof *c);
  if (!c) return NULL;
  c->s = calloc(1, sizeof *c->s);
  if (!c->s) { free(c); return NULL; }
  c->headless = 1;
  coder_page_init(cd_state(c));
  return c;
}
// The chat in from moves to to, run and all; from is left a new chat. 0 when to holds a running chat.
int cd_take(void *to, void *from) {
  CoderHost *t = to, *f = from;
  if (!t || !f) return 0;
  int ok = coder_page_take(cd_state(t), cd_state(f));
  cd_changed(t); cd_changed(f);
  return ok;
}
void cd_tick(void *page, float dt) { CoderHost *c = page; if (c) coder_page_tick(cd_state(c), dt); }
void cd_close(void *page) {
  CoderHost *c = page;
  if (!c) return;
  if (!c->headless) gallery_free(&c->app);
  free(c->s); free(c);
}
