// Firm reason: 6 (opaque SDK lifetime boundary; no generated types cross Swift).
// Coder's page (page.coax) on the shared component host, as a Metal view the app puts in its
// window. Rectangles are in points from the view's top left. Every call is on the main thread.
#ifndef CODER_HOST_H
#define CODER_HOST_H
#include <stdint.h>
// resources: the folder holding the component shader, fonts and icons. NULL when it cannot open.
void *cd_open(const char *resources);
// Open with explicit offscreen mode (1 = no window, for --shot / --smoke) and initial size.
void *cd_open_ex(const char *resources, int offscreen, int width, int height);
void *cd_view(void *page);
// One frame on Linux/Wayland (returns 0 once the window is closed).
int cd_step(void *page);
// Offscreen render and PNG output (when opened with offscreen=1).
int cd_shot_frame(void *page, uint32_t w, uint32_t h, float dt);
int cd_shot_write(void *page, const char *png);
int cd_copy(void *page, const char *bytes, int n);
int cd_focused(void *page);
// One frame. Call it while cd_moving says 1, and after anything changes.
void cd_frame(void *page);
int cd_moving(void *page);
// Called when something changed without the app's doing (a click or the wheel on the page).
void cd_on_change(void *page, void (*changed)(void *context), void *context);
// Whether the view has the keyboard (its window key, it the first responder); reduced motion (bit 1)
// and reduced transparency (bit 2).
void cd_env(void *page, int active, int reduced);
// The page draws and edits its text fields itself (page/editor.coax): the view hands it every key
// and the input method. cd_focus gives the keyboard to the composer (1), settings' search (2) or
// neither (0). cd_wants_text: a field has it. cd_key: a key as the view gives it (a named key below
// zero or a letter, mods 1 Shift, 2 Control, 4 Option, 8 Command), for a chord the menu bar would
// take first (Command-C, -X, -V, -A, -Z); 1 when taken. cd_blink: seconds until the caret blinks
// after the last frame, -1 for none. cd_draft: the composer's text, NUL-ended.
void cd_focus(void *page, int field);
int cd_wants_text(void *page);
int cd_key(void *page, int key, int mods);
float cd_blink(void *page);
int cd_draft(void *page, char *out, int cap);
// The draft set from the app: 0 when it is longer than a draft may be.
int cd_set_text(void *page, const char *text);
// As a press on Send: 1 when the draft was sent (cd_action then says so).
int cd_submit(void *page);
// A new chat, as the New chat button makes one. 0 while a run is going.
int cd_new(void *page);
// What the page asks for, once: 0 nothing, 1 the draft was sent (write cd_task to the worker and
// to the worker), 2 the composer wants the keyboard, 3 stop the run (cd_stop_line), 4 a new chat
// (cd_forget_line), 5 a code block's Copy (cd_copied is its text), 6 choose the working folder.
int cd_action(void *page);
// The worker's lines (coder/tui/mcworker.coax), each with its newline: the length, 0 for none.
int cd_task(void *page, const char *session, const char *cwd, char *out, int cap);
// The note on installed skills that goes ahead of each message (not shown).
void cd_skills(void *page, const char *note);
// The working folder as shown in the corner (~/work/app). Its whole path is cd_task's cwd.
void cd_folder(void *page, const char *shown);
// The working folder's Git branch ("" for none), shown after the folder in the composer's foot.
void cd_branch(void *page, const char *name);
// The answer the worker is owed after a line that asked about a command: its length, 0 for none.
int cd_answer_line(void *page, char *out, int cap);
// Sub-agents: Microcoder hands tasks over, and the app runs each on a worker of its own.
// cd_agent_next: the next one to start a worker for (once each), -1 for none. cd_agent_task: its
// task line. cd_agent_line: a line its worker wrote. cd_agent_failed: its worker could not start
// or went away. cd_agent_answer_line: the yes its worker is owed, 0 for none. cd_agent_running:
// 0 once it has ended (stop its worker). When every one has ended, cd_answer_line gives the
// results line for Microcoder's worker (give it 1,048,576 bytes: the worker's line).
int cd_agent_next(void *page);
int cd_agent_task(void *page, int agent, const char *session, const char *cwd, char *out, int cap);
void cd_agent_line(void *page, int agent, const char *line);
void cd_agent_failed(void *page, int agent, const char *why);
int cd_agent_answer_line(void *page, int agent, char *out, int cap);
int cd_agent_running(void *page, int agent);
int cd_agent_cancel(void *page, int agent);
int cd_stop_agent(void *page);
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
// Show or hide the sidebar keyboard shortcut badges when Command is held.
void cd_cmd_hints(void *page, int shown);
// Update the sidebar footer auto-updater pill (state: 0=hidden, 1=available, 2=downloading, 3=ready, 4=error).
// A toast about a background event at the window's foot (tone 0 plain, 1 a failure; GitHub #13774).
void cd_toast(void *page, const char *text, int tone);
void cd_update(void *page, int state, const char *label, const char *detail);
int cd_open_id(void *page, char *out, int cap);
// The file a press on a row or a reply asked to open (cd_action 24), as it was named: a whole path,
// ~/ and the home folder, or one from the working folder. Its length, 0 for none.
int cd_open_path(void *page, char *out, int cap);
int cd_open_url(void *page, char *out, int cap);
// The draft's attachments (page/attach.coax, GitHub #13699): cd_attach a file chosen in the open
// panel (1 when it joined); cd_attached how many the draft carries; cd_attach_clear takes them
// away (a follow-up sent them); cd_draft_message the draft as a follow-up sends it, its words and
// the note on the files it carries and mentions, NUL-ended (0 when it does not fit); cd_add_at
// where the composer's + is, device pixels, for its menu (cd_action 27); cd_files the working
// folder's files, paths relative to it one a line, for the @ popover (cd_action 28 asks).
int cd_attach(void *page, const char *path);
int cd_attached(void *page);
void cd_attach_clear(void *page);
int cd_draft_message(void *page, char *out, int cap);
int cd_add_at(void *page, float out[2]);
void cd_files(void *page, const char *lines);
// The file in the pane on the right (page/filepane.coax, GitHub #13760). cd_file_most: the bytes it
// shows at most. cd_file_open: a file read (its path as shown, its bytes, its size on disk; the
// carrier keeps a copy for the page); cd_file_failed: one that could not be read, and why.
// cd_file_text: the editor's text to save, its length (-1 when it does not fit or nothing is
// edited); cd_file_saved: the bytes written; cd_file_save_failed: why a save failed; cd_file_close
// closes the pane; cd_file_front brings the open file forward. cd_file_mode: 0 none, 1 edited
// here, 2 read-only, 3 not read; cd_file_dirty: changes not saved; cd_file_shown: it shows now;
// cd_file_name: its name, NUL-ended. cd_action asks 40 save, 41 open it in its own app, 42 close
// (ask first when dirty), 43 keep the keyboard on the view for it.
int cd_file_most(void);
int cd_file_open(void *page, const char *path, const char *bytes, int len, long long size);
void cd_file_failed(void *page, const char *path, const char *why);
int cd_file_text(void *page, char *out, int cap);
void cd_file_saved(void *page, const char *bytes, int len);
void cd_file_save_failed(void *page, const char *why);
void cd_file_close(void *page);
int cd_file_front(void *page);
int cd_file_mode(void *page);
int cd_file_dirty(void *page);
int cd_file_shown(void *page);
int cd_file_name(void *page, char *out, int cap);
int cd_file_open_scene(void *page, const char *path);
int cd_file_scene_rect(void *page, float out[4]);
// The live 3D card on screen (page/shows.coax, #13812): its rect then the conversation's clip (x, y, w, h each,
// points from the top left) into out, its scene's path into path. Returns the path's length; 0 when none shows.
int cd_live_scene(void *page, float out[8], char *path, int cap);
// What the page drew over everything in its last frame (menus, popovers, dialogs, toasts): up to rects rects
// (x, y, w, h each, points from the top left) into out, for the 3D views over the page to cut out (#13817).
int cd_over_rects(void *page, float *out, int rects);
// The iOS Simulator in the pane (page/simpane.coax, GitHub #13811). cd_sim_setup: where stills
// and the person's screenshots go. cd_sim_show: View > iOS Simulator (1 open, 0 close).
// cd_sim_connected: the host reached the helper's socket (1) or could not (0, why). cd_sim_line:
// a line the helper wrote (no line feed); cd_sim_out: the lines the page queued for the helper
// (each ending in a line feed), their length. cd_sim_tick: 15 a second while the screen
// shows. cd_sim_rect: the screen's x, y, w, h (top-left points), covered (1: hide the view) and
// the device frame's width round the screen (the host draws the device's chrome in it).
// cd_sim_still: the newest still's path (NUL-ended), and how many there have been. cd_action 46
// asks; cd_sim_op says what: 1 the device menu (cd_sim_menu rows "title\tstate\tshown", then
// cd_sim_pick), 2 a screenshot saved (cd_sim_saved), 3 the pane closed, 4 it opened.
void cd_sim_setup(void *page, const char *stills, const char *shots);
void cd_sim_show(void *page, int open);
int cd_sim_open(void *page);
void cd_sim_connected(void *page, int ok, const char *why);
int cd_sim_line(void *page, const char *line, int len);
int cd_sim_out(void *page, char *out, int cap);
void cd_sim_tick(void *page);
int cd_sim_rect(void *page, float out[6]);
int cd_sim_still(void *page, char *out, int cap);
int cd_sim_op(void *page);
int cd_sim_saved(void *page, char *out, int cap);
int cd_sim_menu(void *page, char *out, int cap);
int cd_sim_pick(void *page, int i);
int cd_sim_device(void *page, char *out, int cap);
// cd_sim_tips: the pane's icon buttons' tooltips, one a line "x\ty\tw\th\ttext" (top-left points).
int cd_sim_tips(void *page, char *out, int cap);
// The person's hand on the screen's view: cd_sim_touch a finger (phase 0 down, 1 moved, 2 up) at
// x, y from the view's top left in a view w x h; cd_sim_key a key (its Mac key code and the
// characters it types). Each queues a request (cd_sim_out); 0 when the helper has no input.
int cd_sim_touch(void *page, int phase, float x, float y, float w, float h);
int cd_sim_key(void *page, int code, const char *chars);
// cd_sim_pinch two fingers (the pointer and its mirror through the screen's centre: Option-drag);
// cd_sim_scroll a scroll as a dragging finger (phase 0 began, 1 changed, 2 ended; dx, dy view
// points the content follows); cd_sim_command a Command shortcut (1 when it was the simulator's).
int cd_sim_pinch(void *page, int phase, float x, float y, float w, float h);
int cd_sim_scroll(void *page, int phase, float x, float y, float dx, float dy, float w, float h);
int cd_sim_command(void *page, int code, const char *chars, int shift);
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
void cd_agy(void *page, int on);
int cd_model_agy(void *page);
int cd_model_inkling(void *page);
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
// Follow-ups (GitHub #13715, #13716). The worker owns the queue and a running turn's steers; these
// write its lines and read back what it last said. ids are the host's (a UUID each), stable across
// a reconnect: the same id twice changes nothing.
// cd_steer_line: input for the running turn ({"steer"...}), 0 when no worker turn runs.
// cd_queue_message_line: op "add" or "update" with the message's whole task line inside.
// cd_queue_line: op open (key = the chat id), delete, move (to), promote (now: start it now),
// pause, resume, clear, list. cd_interrupt_line: stop the turn; resubmit sends its pending steers
// as a new turn. cd_followup_began: 1 once when the worker started a turn on its own (keep the
// chat as for a sent message, write nothing). cd_queue_waiting: items wait and the queue is not
// paused (keep the worker). cd_steer_status: 0 sent, 1 accepted, 2 rejected (cd_steer_reason:
// no_active_turn, expected_turn_mismatch, active_turn_not_steerable, invalid, full), 3 applied,
// 4 queued (moved to the queue's head), 5 resubmitted. The text reads end with a zero byte.
int cd_steer_line(void *page, const char *id, const char *text, char *out, int cap);
int cd_queue_message_line(void *page, const char *op, const char *id, const char *text, const char *session, const char *cwd, char *out, int cap);
int cd_queue_line(void *page, const char *op, const char *id, int to, int now, const char *key, char *out, int cap);
int cd_interrupt_line(void *page, int resubmit, char *out, int cap);
int cd_followup_began(void *page);
int cd_queue_waiting(void *page);
int cd_queue_count(void *page);
int cd_queue_paused(void *page);
int cd_queue_reason(void *page, char *out, int cap);
int cd_queue_item_id(void *page, int i, char *out, int cap);
int cd_queue_item_text(void *page, int i, char *out, int cap);
int cd_queue_item_cut(void *page, int i);
int cd_steer_count(void *page);
int cd_steer_status(void *page, int i);
int cd_steer_text(void *page, int i, char *out, int cap);
int cd_steer_reason(void *page, int i, char *out, int cap);
// The tray and the composer (GitHub #13717). cd_followup_submit: Return (queue=0, steers) or
// Shift-Return (queue=1, queues) while a reply runs or a queued item is edited; 1 when it asked
// the host (cd_action 25), 0 when Return sends the draft (cd_submit) or Shift-Return inserts a
// new line. cd_followup_escape: Esc gives up an edit or stops the reply; 1 when it did.
// cd_followup_op / cd_followup_item: what cd_action 25 asks: 1 resume, 2 clear, 3 delete, 4 move
// up, 5 edit, 6 send (steer it in while a reply runs, else promote now), 7 send the waiting steer
// now (interrupt with resubmit), 9 queue a steer not taken; 20 steer the draft, 21 queue the
// draft, 22 save the draft as the edited item, 23 give up the edit, 24 interrupt. item: the
// queued item's place, or the steer's. cd_followup_editing: the queued item the composer holds
// (-1 none); cd_followup_edited reads it. cd_followup_mode / cd_followup_queues: kept for ABI
// compatibility (Return always steers). cd_steer_id: steer i's id. cd_steer_dismiss: steer i
// leaves the tray.
int cd_followup_submit(void *page, int other);
int cd_followup_escape(void *page);
int cd_followup_op(void *page);
int cd_followup_item(void *page);
void cd_followup_editing(void *page, int i);
int cd_followup_edited(void *page);
void cd_followup_mode(void *page, int queue_first);
int cd_followup_queues(void *page);
int cd_steer_id(void *page, int i, char *out, int cap);
void cd_steer_dismiss(void *page, int i);
int cd_worker_turn(void *page);
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
// cd_box: the drawn box (points: x, y from the content's top-left, w, h) of the row with this id; 0 when none shows.
int cd_box(void *page, const char *id, float box[4]);
// The text size: a row of 0 to 7, 2 the usual. cd_zoom moves it by rows (0 leaves it), cd_zoom_set
// names the row. Each returns the row it is at.
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
// Claude key's and the OpenRouter key's state as shown ("" not set) and the app's version.
// cd_settings_tier: the tier a press on Foreman's page chose (cd_action 18). cd_search_set: the
// search's text. cd_search_submit: Return in it; 0 when it found nothing. cd_search_clear: Esc,
// a search with words emptied (0 when it was empty).
// cd_action asks: 13 settings opened or closed (move the keyboard), 14/15/29 text size up/down/reset,
// 16 the Claude key, 17 OpenRouter, 18 Foreman's tier, 19 its audit log, 20 clear its history,
// 21 log in (the web page), 22 check for updates, 23 the search field wants the keyboard,
// 33 Antigravity CLI/model/effort, 34 Antigravity review mode, 35 Antigravity sign-in/profile.
void cd_settings_open(void *page, int which, const char *row);
int cd_settings_page(void *page);
void cd_settings_info(void *page, const char *claude, const char *openrouter, const char *version);
void cd_settings_gemini(void *page, const char *gemini);
void cd_settings_local(void *page, const char *status);
void cd_inkling(void *page, int on);
// cd_settings_instructions: the agent instructions' state General shows ("Default", "Edited"; #13808).
void cd_settings_instructions(void *page, const char *words);
// The sign-in Gemini models run on, from mcworker --auth: kind "vertex", "key" or "none", and its words.
void cd_settings_gauth(void *page, const char *kind, const char *text);
// The model agy opens a session on with the current settings (agyworker --probe): its id and name.
void cd_agy_seen(void *page, const char *id, const char *name);
void cd_settings_agy(void *page, const char *status, int mode, const char *model, const char *auth);
int cd_settings_agy_mode(void *page);
int cd_settings_tier(void *page);
void cd_search_set(void *page, const char *text);
int cd_search_submit(void *page);
int cd_search_clear(void *page);
// The window's title bar (page/layout/titlebar.coax, GitHub #13741). cd_bar_drags: whether a press at
// (x, y), window points from its top left, is on the bar where nothing of the page's takes it (the
// window moves). cd_bar_full: the window went into full screen (1) or came out (0).
int cd_bar_drags(void *page, float x, float y);
void cd_bar_full(void *page, int on);
// Dialogs and right-click menus drawn over the page (page/overlay.coax, GitHub #13746).
// A dialog: cd_dialog_begin (size 0 sm, 1 md, 2 lg), its fields (secure; lines 1 a line, more a
// box) and buttons in the foot's order (kind 0 primary, 1 secondary, 2 destructive; aside at the
// foot's start), then cd_dialog_show with the buttons Return and Escape stand for (-1 none) and the field the keyboard
// starts on.
// cd_dialog_answer: the button pressed, -1 cancelled, -2 none yet (once). cd_dialog_value: a
// field's text, NUL-ended (-1 when it does not fit). cd_dialog_set: a field's text.
// The field with the keyboard is the page's editor (cd_key, the input method): a secure one shows dots and never copies.
// A menu: cd_menu_begin, its items (kind 0 row, 1 heading, 2 rule), cd_menu_show at the pointer in
// the canvas's device pixels. cd_menu_answer: the item chosen, -1 none, -2 not yet (once).
// While one is up it takes the pointer (cd_on_overlay's function is called after each pointer
// event) and the wheel; cd_overlay_key gives it a key (-1 Escape, -2 Return, -6 up, -7 down,
// -13 Tab, -3 Backspace, which on the palette's empty field goes back; mods 1 Shift, 8 Command): 0 leaves the key to the field with the keyboard.
void cd_on_overlay(void *page, void (*fn)(void *context), void *context);
// The command palette (page/palette.coax, GitHub #13700), drawn as the dialogs are; its field is
// the page's fourth editor (page/editor.coax), typed into as the others are. cd_palette_toggle: ⌘K opens it in a
// scope (0 All, 1 Sessions, 2 Commands, 3 Settings) or closes it; whether it is up now.
// cd_palette_command: what cd_action 26 asks (0 watch the palette that opened, 16 rename the chat
// shown, 17 delete it, 18 stop every running chat). cd_chat_step: ⌃Tab (1) and ⌃⇧Tab (-1), then
// cd_action opens the chat. cd_toggle_sidebar: ⌘B. cd_toggle_models: ⌘/.
int cd_palette_toggle(void *page, int scope);
int cd_palette_open(void *page);
int cd_palette_command(void *page);
int cd_chat_step(void *page, int dir);
void cd_toggle_sidebar(void *page);
void cd_toggle_models(void *page);
void cd_dialog_begin(void *page, int size, const char *title, const char *body);
int cd_dialog_field(void *page, const char *label, const char *value, const char *hint, int secure, int lines);
int cd_dialog_button(void *page, const char *label, int kind, int aside);
void cd_dialog_show(void *page, int confirm, int cancel, int focus);
void cd_dialog_select_all(void *page);
int cd_dialog_answer(void *page);
int cd_dialog_value(void *page, int i, char *out, int cap);
int cd_dialog_set(void *page, int i, const char *text);
void cd_menu_begin(void *page);
int cd_menu_item(void *page, const char *label, int kind, int checked, int disabled, int danger);
void cd_menu_show(void *page, float x, float y);
int cd_menu_answer(void *page);
int cd_overlay_open(void *page);
int cd_overlay_key(void *page, int key, int mods);
void cd_gym_set(void *page, int open);
int cd_gym_open(void *page);
int cd_gym_rect(void *page, float out[4]);
// Experimental features (log in and the account, Gym, Foreman): set by the host from UserDefaults; read after CP_ASK_EXPERIMENTAL.
void cd_experimental_set(void *page, int on);
int cd_experimental(void *page);
void cd_cloud_authed_set(void *page, int on);
// Git Actions toolbar (page/gitactions.coax, GitHub #13871). cd_action 49 asks; cd_git_op says which op.
void cd_git_status(void *page, int in_repo, int in_worktree, int dirty, int ahead, int behind);
void cd_git_mode_set(void *page, int mode);
int cd_git_mode(void *page);
int cd_git_branches(void *page, const char *lines);
void cd_git_busy(void *page, int busy, const char *label);
void cd_git_output_clear(void *page);
void cd_git_output_append(void *page, const char *chunk);
void cd_git_output_done(void *page, int failed);
void cd_git_wt_status(void *page, int status, const char *note);
int cd_git_op(void *page);
int cd_git_anchor(void *page, float out[2]);
int cd_git_needs_guard(const char *branch, int op);
int cd_git_valid_branch(const char *name);
int cd_git_valid_commit(const char *msg);
// Live Git diff viewer, changed-files tree, and inline review comments (page/gitdiff.coax, GitHub #13870).
int cd_git_diff_max(void);
void cd_git_diff_set(void *page, const char *bytes, int len);
int cd_git_diff_files(void *page);
int cd_review_pick(void *page, char *path, int path_cap, char *snippet, int snip_cap, char *body, int body_cap, int meta[4]);
int cd_review_comment_add(void *page, const char *path, int start_line, int end_line, int side, const char *snippet, const char *body);
int cd_review_comment_edit(void *page, int id, const char *body);
int cd_review_comment_delete(void *page, int id);
int cd_review_comments_submit(void *page);
int cd_review_comments_count(void *page);
#endif
