// Firm reason: 6 (opaque C interface between the compiled Coax GPU iOS surface and Swift UIKit host).
#ifndef OPENAGENTS_IOS_GPU_HOST_H
#define OPENAGENTS_IOS_GPU_HOST_H

#include <stdint.h>

// Transcript item roles matching gpu-page.coax IOS_ROLE_*
#define OA_IOS_ROLE_USER 0
#define OA_IOS_ROLE_ASSISTANT 1
#define OA_IOS_ROLE_THOUGHT 2
#define OA_IOS_ROLE_TOOL 3
#define OA_IOS_ROLE_NOTICE 4

// Reachability states matching gpu-page.coax IOS_REACH_*
#define OA_IOS_REACH_UNKNOWN 0
#define OA_IOS_REACH_CHECKING 1
#define OA_IOS_REACH_OK 2
#define OA_IOS_REACH_BAD 3

// Actions emitted to Swift matching gpu-page.coax IOS_ACT_*
#define OA_IOS_ACT_NONE 0
#define OA_IOS_ACT_FOCUS_COMPOSER 1
#define OA_IOS_ACT_SEND 2
#define OA_IOS_ACT_STOP 3
#define OA_IOS_ACT_NEW_CHAT 4
#define OA_IOS_ACT_SELECT_SESSION 5
#define OA_IOS_ACT_OPEN_COMPUTERS 6
#define OA_IOS_ACT_OPEN_WORKSPACE 7
#define OA_IOS_ACT_OPEN_SUBAGENT 8
#define OA_IOS_ACT_OPEN_HUB 9
#define OA_IOS_ACT_OPEN_ATTACH 10
#define OA_IOS_ACT_CLEAR_ATTACH 11
#define OA_IOS_ACT_CHOSE_MODEL 12
#define OA_IOS_ACT_CHOSE_EFFORT 13
#define OA_IOS_ACT_CHOSE_MODE 14
#define OA_IOS_ACT_TOGGLE_MIC 15
#define OA_IOS_ACT_APPROVE 16
#define OA_IOS_ACT_DENY 17
#define OA_IOS_ACT_DISMISS_ERROR 18
#define OA_IOS_ACT_RETRY_OUTBOX 19
#define OA_IOS_ACT_COPY_TEXT 20
#define OA_IOS_ACT_OPEN_FILE 21
#define OA_IOS_ACT_LOAD_OLDER 22
#define OA_IOS_ACT_LOAD_MORE_SESSIONS 23
#define OA_IOS_ACT_APPLY_SUGGESTION 24
#define OA_IOS_ACT_APPROVAL_OPTION 25
#define OA_IOS_ACT_CLEAR_QUEUED 26
#define OA_IOS_ACT_CHOSE_FOLDER 27
#define OA_IOS_ACT_DASHBOARD_CHANGED 28

// Todo item status matching gpu-page.coax IOS_TODO_*
#define OA_IOS_TODO_PENDING 0
#define OA_IOS_TODO_IN_PROGRESS 1
#define OA_IOS_TODO_COMPLETED 2

void *oa_ios_open(const char *resources);
void oa_ios_close(void *surface);
void *oa_ios_view(void *surface);
void oa_ios_frame(void *surface);
int oa_ios_moving(void *surface);
void oa_ios_on_change(void *surface, void (*changed)(void *context), void *context);

void oa_ios_env(void *surface, float safe_top, float safe_bottom, float keyboard, int focused, float editor_height, int reduced);
void oa_ios_transparency(void *surface, int reduced);
float oa_ios_text_zoom(void *surface, float zoom);

void oa_ios_native_scroll(void *surface, int on);
int oa_ios_scroll_info(void *surface, float out[3]);
void oa_ios_scroll_to(void *surface, float points);
int oa_ios_scroll_page(void *surface, int dir);

int oa_ios_editor(void *surface, float box[6]);
int oa_ios_style(float style[14]);
int oa_ios_focus(void *surface, char *out, int cap);
void oa_ios_set_focus(void *surface, const char *id);
void oa_ios_blur(void *surface);
int oa_ios_draft_set(void *surface, const char *text);
int oa_ios_draft_get(void *surface, char *out, int cap);

int oa_ios_rows(void *surface);
int oa_ios_rows_stamp(void *surface);
int oa_ios_layer(void *surface);
int oa_ios_row(void *surface, int i, float row[8], char *id, int id_cap, char *label, int label_cap);
int oa_ios_row_value(void *surface, int i, char *out, int cap);
int oa_ios_activate(void *surface, const char *id);

int oa_ios_take_action(void *surface);
int oa_ios_action_arg(void *surface, char *out, int cap);
int oa_ios_copy_get(void *surface, char *out, int cap);
void oa_ios_drawer_set(void *surface, int open);
int oa_ios_drawer_open(void *surface);
void oa_ios_dashboard_set(void *surface, int open);
int oa_ios_dashboard_open(void *surface);
void oa_ios_scroll_top(void *surface);
void oa_ios_scroll_bottom(void *surface);

void oa_ios_set_header(void *surface, const char *computer, const char *session_title, const char *workspace, int reachability, int paired_count, int has_active, int has_older, int has_more_sessions);
void oa_ios_set_banners(void *surface, const char *error, int outbox_count, const char *toast);
void oa_ios_set_approval(void *surface, int active, const char *prompt, const char *warning);
void oa_ios_approval_options_clear(void *surface);
void oa_ios_approval_options_push(void *surface, const char *id, const char *label, const char *kind);
void oa_ios_set_composer(void *surface, const char *model_label, const char *effort_label, const char *mode_label, int attachments, int mic_active);
void oa_ios_set_working(void *surface, int busy, int awaiting, int secs, const char *label);
void oa_ios_set_usage(void *surface, const char *summary, int context_pct);
void oa_ios_set_queued(void *surface, const char *text);

void oa_ios_suggestions_clear(void *surface, int is_mention);
void oa_ios_suggestions_push(void *surface, const char *id, const char *label, const char *detail);

void oa_ios_todos_clear(void *surface);
void oa_ios_todos_push(void *surface, const char *content, int status);

void oa_ios_folders_clear(void *surface);
void oa_ios_folders_push(void *surface, const char *path, const char *label, int selected);

void oa_ios_sessions_clear(void *surface);
void oa_ios_sessions_push(void *surface, const char *id, const char *title, const char *repo, const char *branch, const char *status, int busy, int current, int attention, int unread);
void oa_ios_session_set_preview(void *surface, const char *id, const char *ask, const char *activity, const char *reply);

void oa_ios_items_clear(void *surface, int reset_scroll);
void oa_ios_items_push(void *surface, int role, const char *id, const char *text, const char *tool, const char *path, const char *diff, int complete, int failed, int is_subagent, int diff_cut, int subagent_calls, int elapsed_secs);

void oa_ios_models_clear(void *surface);
void oa_ios_models_push(void *surface, const char *id, const char *label, const char *detail, int selected);
void oa_ios_efforts_clear(void *surface);
void oa_ios_efforts_push(void *surface, const char *id, const char *label, const char *detail, int selected);
void oa_ios_modes_clear(void *surface);
void oa_ios_modes_push(void *surface, const char *id, const char *label, const char *detail, int selected);

#endif
