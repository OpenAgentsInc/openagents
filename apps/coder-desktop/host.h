// Firm reason: 6 (opaque SDK lifetime boundary; no generated types cross Swift).
// Coder's page (page.coax) on the shared component host, as a Metal view the app puts in its
// window. Rectangles are in points from the view's top left. Every call is on the main thread.
#ifndef CODER_HOST_H
#define CODER_HOST_H
#include <stdint.h>
// resources: the folder holding the component shader, fonts and icons. NULL when it cannot open.
void *cd_open(const char *resources);
void *cd_view(void *page);
// One frame. Call it while cd_moving says 1, and after anything changes.
void cd_frame(void *page);
int cd_moving(void *page);
// Called when something changed without the app's doing (a click or the wheel on the page).
void cd_on_change(void *page, void (*changed)(void *context), void *context);
// The platform editor: whether it holds the keyboard, its own text height in points while it does; reduced motion.
void cd_env(void *page, int focused, float editor_height, int reduced);
// The editor's whole text after an edit: 0 when it is longer than a draft may be.
int cd_set_text(void *page, const char *text);
// Return in the editor: 1 when the draft was sent (cd_action then says so).
int cd_submit(void *page);
// A new chat, as the New chat button makes one. 0 while a run is going.
int cd_new(void *page);
// What the page asks for, once: 0 nothing, 1 the draft was sent (write cd_task to the worker and
// empty the editor), 2 the text wants the keyboard, 3 stop the run (cd_stop_line), 4 a new chat
// (cd_forget_line), 5 a code block's Copy (cd_copied is its text), 6 choose the working folder.
int cd_action(void *page);
// The worker's lines (coder/tui/mcworker.coax), each with its newline: the length, 0 for none.
int cd_task(void *page, const char *session, const char *cwd, char *out, int cap);
// The note on installed skills that goes ahead of each message (not shown).
void cd_skills(void *page, const char *note);
// The working folder as shown in the corner (~/work/app). Its whole path is cd_task's cwd.
void cd_folder(void *page, const char *shown);
// The answer the worker is owed after a line that asked about a command: its length, 0 for none.
int cd_answer_line(void *page, char *out, int cap);
// Sub-agents: Microcoder hands tasks over, and the app runs each on a worker of its own.
// cd_agent_next: the next one to start a worker for (once each), -1 for none. cd_agent_task: its
// task line. cd_agent_line: a line its worker wrote. cd_agent_failed: its worker could not start
// or went away. cd_agent_answer_line: the yes its worker is owed, 0 for none. cd_agent_running:
// 0 once it has ended (stop its worker). When every one has ended, cd_answer_line gives the
// results line for Microcoder's worker (give it 300,000 bytes).
int cd_agent_next(void *page);
int cd_agent_task(void *page, int agent, const char *session, const char *cwd, char *out, int cap);
void cd_agent_line(void *page, int agent, const char *line);
void cd_agent_failed(void *page, int agent, const char *why);
int cd_agent_answer_line(void *page, int agent, char *out, int cap);
int cd_agent_running(void *page, int agent);
// Kept chats. cd_chats: the list for the sidebar, most recent first, one chat a line (its id, a
// tab, its title). cd_current: the id of the chat that is shown ("" for one not kept yet).
// cd_open_id: the id a press in the sidebar asked for (cd_action 7). cd_save_due: the chat changed
// and is to be kept (once). cd_save: its text; cd_title: its title; each ends with a zero byte and
// returns its length, 0 when it does not fit. cd_load: a kept text becomes the chat that is shown.
// cd_recall_line: the line that tells the worker the opened chat's turns, 0 for none.
void cd_chats(void *page, const char *list);
void cd_current(void *page, const char *id);
// Update the persistent Foreman sidebar row's running spinner and permission tier (0=Observe, 1=Assist, 2=Auto).
void cd_foreman(void *page, int busy, int tier);
// Update the sidebar footer auto-updater pill (state: 0=hidden, 1=available, 2=downloading, 3=ready, 4=error).
void cd_update(void *page, int state, const char *label, const char *detail);
int cd_open_id(void *page, char *out, int cap);
// The file a press on a row or a reply asked to open (cd_action 24), as it was named: a whole path,
// ~/ and the home folder, or one from the working folder. Its length, 0 for none.
int cd_open_path(void *page, char *out, int cap);
// The pinned chat id and new 0-based pinned slot after a drag reorder (action 10).
int cd_pin_move(void *page, char *id_out, int cap, int *to_out);
int cd_title(void *page, char *out, int cap);
// The id of the chat whose sidebar row is at (x, y) device pixels, 0 when none is there.
int cd_chat_at(void *page, float x, float y, char *out, int cap);
// Nonzero when the persistent Foreman sidebar row is at (x, y) device pixels.
int cd_foreman_at(void *page, float x, float y);
int cd_save_due(void *page);
int cd_save(void *page, char *out, int cap);
int cd_load(void *page, const char *text);
int cd_recall_line(void *page, char *out, int cap);
// cd_claude: whether Claude Code is installed here (with it, Microcoder may hand a task to a Claude
// model). cd_agent_claude: whether this sub-agent runs in Claude Code, on the Claude worker.
void cd_claude(void *page, int on);
// cd_openrouter: the OpenRouter models set with Coder > OpenRouter Models ("" for none): main is a
// composer picker entry for Microcoder, sub is a sub-agent level (openrouter).
void cd_openrouter(void *page, const char *main, const char *sub);
// cd_pick_model: the shown chat's model by name (a launch's --model); 0 when no entry has that name.
int cd_pick_model(void *page, const char *name);
int cd_agent_claude(void *page, int agent);
// Opens the transcript of sub-agent n (1 is the first) of the newest message that has that many, on
// the window's right, as a press on its row does. 0 when no message has.
int cd_pane(void *page, int n);
// The sidebars' widths as dragged, in points at the usual text size; 0 is one never dragged.
// cd_sides is 1 once after a drag ends: keep them then.
int cd_sides(void *page, float out[2]);
void cd_sides_set(void *page, float left, float right);
int cd_stop_line(char *out, int cap);
int cd_forget_line(char *out, int cap);
// One line the worker wrote, without its newline.
void cd_line(void *page, const char *line);
// The worker could not start or went away: the running message says why.
void cd_failed(void *page, const char *why);
int cd_running(void *page);
// The chat's running total cost in microdollars (1,000,000 = $1.00), updated in real time as attempts arrive.
int64_t cd_cost_usd_micros(void *page);
int64_t cd_budget_usd_micros(void *page);
int cd_set_budget_usd_micros(void *page, int64_t micros);
int cd_budget_exceeded(void *page);
int cd_budget_approve(void *page);
// Active thread goal (/goal): status (0=none, 1=active, 2=paused, 3=complete, 4=blocked, 5=budget_limited).
int cd_goal_status(void *page);
int cd_goal_time(void *page);
int cd_goal_objective(void *page, char *out, int cap);
int cd_goal_set(void *page, const char *objective);
int cd_goal_set_status(void *page, int status);
int cd_goal_clear(void *page);
int cd_copied(void *page, char *out, int cap);
// The text's committed box (x, y, w, h, the first line's inset, a line's height), or 0 before the
// first frame; the editor's style from the skin (14 floats, as apps/shared/composer/host.h's gc_style).
int cd_editor(void *page, float box[6]);
int cd_style(void *page, float style[14]);
// The text size: a row of 0 to 7, 2 the usual. cd_zoom moves it by rows (0 leaves it), cd_zoom_set
// names the row. Each returns the row it is at. Read cd_style again after a change.
int cd_zoom(void *page, int by);
int cd_zoom_set(void *page, int row);
// A page with no window, for a chat whose reply goes on out of view: every cd_ call that reads
// lines, answers workers or keeps text works on it. cd_take moves a chat, run and all, from one
// page to another and leaves the first a new chat (0 when the other holds a running chat).
// cd_tick counts seconds on a page with no window. cd_close frees either kind.
void *cd_headless(void);
int cd_take(void *to, void *from);
void cd_tick(void *page, float dt);
void cd_close(void *page);
// Settings (settings.coax). cd_settings_open: open it on a page (0 General, 1 Models, 2 Foreman,
// 3 Account, 4 About & updates; -1 the last one shown), scrolled to a row id ("" the top); a page
// below -1 closes it. cd_settings_page: the page it shows, -1 while closed. cd_settings_info: the
// Claude key's and the OpenRouter key's state as shown ("" not set), the app's version, and the
// window's title bar over the page in points.
// cd_settings_tier: the tier a press on Foreman's page chose (cd_action 18). cd_search_box: where
// the search editor goes, in points (0 while settings is closed). cd_search_set: its text after an
// edit. cd_search_submit: Return in it; 0 when it found nothing.
// cd_action asks: 13 settings opened or closed (move the keyboard), 14/15 text size up/down,
// 16 the Claude key, 17 OpenRouter, 18 Foreman's tier, 19 its audit log, 20 clear its history,
// 21 log in (the web page), 22 check for updates, 23 the search field wants the keyboard.
void cd_settings_open(void *page, int which, const char *row);
int cd_settings_page(void *page);
void cd_settings_info(void *page, const char *claude, const char *openrouter, const char *version, float titlebar);
int cd_settings_tier(void *page);
int cd_search_box(void *page, float out[4]);
void cd_search_set(void *page, const char *text);
int cd_search_submit(void *page);
#endif
