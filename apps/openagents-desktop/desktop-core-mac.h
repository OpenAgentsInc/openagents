/* C boundary for apps/openagents-desktop/desktop-core.coax (GitHub #13794). */
#pragma once
#include <stdbool.h>
#include <stdint.h>
#ifndef COAX_STRING_DEFINED
#define COAX_STRING_DEFINED
typedef struct { const char *data; int64_t count; } coax_string;
#endif
#ifndef COAX_coax_v_u8_DEFINED
#define COAX_coax_v_u8_DEFINED
typedef struct { uint8_t *data; int64_t count; } coax_v_u8;
#endif
#ifndef COAX_coax_v_s64_DEFINED
#define COAX_coax_v_s64_DEFINED
typedef struct { int64_t *data; int64_t count; } coax_v_s64;
int64_t oa_dc_wt_clone_paths(coax_string repo, coax_v_u8 out);
#endif
int64_t oa_dc_update_verify(coax_string envelope, coax_string key_ids, coax_string key_hexes, coax_string current, int64_t current_build, int64_t allow_file, coax_v_u8 out_version, coax_v_s64 out_build, coax_v_s64 out_has_build, coax_v_u8 out_bundle, coax_v_u8 out_team, coax_v_u8 out_url, coax_v_u8 out_sha256, coax_v_s64 out_size, coax_v_u8 out_notes, coax_v_u8 out_err);
int64_t oa_dc_update_newer(coax_string candidate, int64_t has_build, int64_t build, coax_string current, int64_t current_build);
int64_t oa_dc_update_archive(coax_string path, int64_t expected_size, coax_string expected_sha256, coax_v_u8 out_err);
int64_t oa_dc_semver_cmp(coax_string a, coax_string b);
int64_t oa_dc_worktrees_on(void);
int64_t oa_dc_wt_root(coax_v_u8 out);
int64_t oa_dc_wt_slug(coax_string repo, coax_v_u8 out);
int64_t oa_dc_wt_real(coax_string path, coax_v_u8 out);
int64_t oa_dc_wt_branch(coax_string chat, coax_v_u8 out);
int64_t oa_dc_wt_shown(coax_string branch, coax_v_u8 out);
int64_t oa_dc_wt_path(coax_string repo, coax_string chat, coax_string root, coax_v_u8 out);
int64_t oa_dc_wt_main_checkout(coax_string folder, coax_v_u8 out);
int64_t oa_dc_wt_repository(coax_string folder, coax_v_u8 out);
int64_t oa_dc_wt_cache_env(coax_string folder, coax_v_u8 out);
int64_t oa_dc_wt_create(coax_string repo, coax_string chat, coax_string root, coax_v_u8 out_path, coax_v_u8 out_source, coax_v_u8 out_branch, coax_v_u8 out_why);
int64_t oa_dc_wt_make(coax_string chat, coax_string path, coax_string source, coax_string branch, coax_v_u8 out_path, coax_v_u8 out_source, coax_v_u8 out_branch, coax_v_u8 out_why);
int64_t oa_dc_wt_hydrate(coax_string chat, coax_string path, coax_string source, coax_string branch);
int64_t oa_dc_wt_changes(coax_string chat, coax_string path, coax_string source, coax_string branch);
int64_t oa_dc_wt_remove(coax_string chat, coax_string path, coax_string source, coax_string branch, coax_v_s64 out_changes);
int64_t oa_dc_new_id(coax_v_u8 out);
int64_t oa_dc_daemon_dir(coax_v_u8 out);
int64_t oa_dc_daemon_socket(coax_string dir, coax_v_u8 out);
int64_t oa_dc_daemon_available(coax_string dir);
int64_t oa_dc_daemon_ensure(coax_string dir, coax_string program, int64_t wait_ms, coax_v_s64 out_pid);
int64_t oa_dc_daemon_probe_runs(coax_string dir);
int64_t oa_dc_daemon_start_run(coax_string dir, coax_string id, coax_v_u8 out_why);
int64_t oa_dc_daemon_stop_run(coax_string dir, coax_string id);
int64_t oa_dc_daemon_forget_run(coax_string dir, coax_string id);
int64_t oa_dc_daemon_update_foreman(coax_string dir, int64_t tier, coax_string prompt);
int64_t oa_dc_daemon_update_meta(coax_string dir, coax_string session, coax_string title, coax_string folder, int64_t pinned, int64_t archived);
int64_t oa_dc_daemon_rpc(coax_string dir, coax_string req_json, coax_v_u8 out_resp);
int64_t oa_dc_run_spec(coax_string program, coax_string home, coax_string env_set_lines, coax_string env_unset_lines, coax_v_u8 out_spec);
int64_t oa_dc_chats_dir(coax_v_u8 out);
int64_t oa_dc_coder_file(coax_string name, coax_v_u8 out);
int64_t oa_dc_clean_title(coax_string raw, coax_v_u8 out);
int64_t oa_dc_store_text(coax_string dir, coax_string id, coax_v_u8 out);
int64_t oa_dc_store_keep(coax_string dir, coax_string id, coax_string title, coax_string folder, coax_string text, coax_string managed, coax_string wt_path, coax_string wt_branch, coax_string repo);
int64_t oa_dc_store_register(coax_string dir, coax_string id, coax_string title, coax_string folder, coax_string managed);
int64_t oa_dc_store_rename(coax_string dir, coax_string id, coax_string title);
int64_t oa_dc_store_toggle_pin(coax_string dir, coax_string id);
int64_t oa_dc_store_remove(coax_string dir, coax_string id);
int64_t oa_dc_store_index_read(coax_string dir, coax_v_u8 out);
int64_t oa_dc_key_read(coax_string name, coax_v_u8 out);
int64_t oa_dc_key_save(coax_string name, coax_string key);
int64_t oa_dc_claude_key(coax_v_u8 out);
int64_t oa_dc_key_tail(coax_string key, coax_v_u8 out);
int64_t oa_dc_worker_path(coax_string home, coax_string had, coax_v_u8 out);
int64_t oa_dc_tools_note(coax_string ple_run, coax_v_u8 out);
int64_t oa_dc_gym_plan(coax_string req, coax_v_u8 out);
int64_t oa_dc_run_file(coax_string daemon_dir, coax_string id, coax_string ext, coax_v_u8 out);
int64_t oa_dc_run_state(coax_string daemon_dir, coax_string id, coax_v_u8 out);
int64_t oa_dc_worker_start_run(coax_string daemon_dir, coax_string program_path, coax_string program_name, coax_string folder, coax_string extra_env, coax_string home, coax_string had_path, coax_string claude_key, coax_v_u8 out_run_id, coax_v_u8 out_failure);
int64_t oa_dc_worker_write_run(coax_string daemon_dir, coax_string run_id, coax_string line);
int64_t oa_dc_worker_touch_run(coax_string daemon_dir, coax_string run_id, int64_t force);
int64_t oa_dc_worker_detach_run(coax_string daemon_dir, coax_string run_id);
int64_t oa_dc_worker_stop_run(coax_string daemon_dir, coax_string run_id, int64_t forget);
int64_t oa_dc_instructions_path(coax_v_u8 out);
int64_t oa_dc_instructions_seed(void);
int64_t oa_dc_instructions_reset(void);
int64_t oa_dc_instructions_read(coax_v_u8 out);
int64_t oa_dc_instructions_write(coax_string text);
int64_t oa_dc_instructions_words(coax_v_u8 out);
int64_t oa_dc_instructions_max(void);
int64_t oa_dc_exec_scan(coax_string home, int64_t ram_mb, coax_v_u8 out_status, coax_v_u8 out_lineage, coax_v_u8 out_badge, coax_v_u8 out_details, coax_v_s64 out_counts);
int64_t oa_dc_pylon_dashboard(coax_string home, coax_v_u8 out_pylon, coax_v_u8 out_contrib, coax_v_u8 out_sats, coax_v_u8 out_release, coax_v_u8 out_details, coax_v_s64 out_counts);
int64_t oa_dc_exec_release_stage(coax_string home, coax_string manifest_path, coax_string payload_path, coax_string trusted_pubkey, coax_v_u8 out_err);
int64_t oa_dc_exec_release_promote(coax_string home, coax_v_u8 out_err);
int64_t oa_dc_exec_release_rollback(coax_string home, coax_v_u8 out_err);
