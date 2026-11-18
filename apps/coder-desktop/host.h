// Firm reason: 6 (opaque SDK lifetime boundary; no generated types cross Swift).
// Coder's page (page.coax) on the shared component host, as a Metal view the app puts in its
// window. Rectangles are in points from the view's top left. Every call is on the main thread.
#ifndef CODER_HOST_H
#define CODER_HOST_H
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
int cd_open_id(void *page, char *out, int cap);
int cd_title(void *page, char *out, int cap);
// The id of the chat whose sidebar row is at (x, y) device pixels, 0 when none is there.
int cd_chat_at(void *page, float x, float y, char *out, int cap);
int cd_save_due(void *page);
int cd_save(void *page, char *out, int cap);
int cd_load(void *page, const char *text);
int cd_recall_line(void *page, char *out, int cap);
// cd_claude: whether Claude Code is installed here (with it, Microcoder may hand a task to a Claude
// model). cd_agent_claude: whether this sub-agent runs in Claude Code, on the Claude worker.
void cd_claude(void *page, int on);
int cd_agent_claude(void *page, int agent);
// Opens the transcript of sub-agent n (1 is the first) of the newest message that has that many, on
// the window's right, as a press on its row does. 0 when no message has.
int cd_pane(void *page, int n);
int cd_stop_line(char *out, int cap);
int cd_forget_line(char *out, int cap);
// One line the worker wrote, without its newline.
void cd_line(void *page, const char *line);
// The worker could not start or went away: the running message says why.
void cd_failed(void *page, const char *why);
int cd_running(void *page);
int cd_copied(void *page, char *out, int cap);
// The text's committed box (x, y, w, h, the first line's inset, a line's height), or 0 before the
// first frame; the editor's style from the skin (14 floats, as apps/shared/composer/host.h's gc_style).
int cd_editor(void *page, float box[6]);
int cd_style(void *page, float style[14]);
// The text size: a row of 0 to 7, 2 the usual. cd_zoom moves it by rows (0 leaves it), cd_zoom_set
// names the row. Each returns the row it is at. Read cd_style again after a change.
int cd_zoom(void *page, int by);
int cd_zoom_set(void *page, int row);
void cd_close(void *page);
#endif
