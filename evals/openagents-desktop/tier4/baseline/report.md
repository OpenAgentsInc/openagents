# Coder Desktop Tier 4 run (47deb95f4d)

Models: gemini-4-hydrogen, gemini-4-neon

Passed 29/29, cost $1.1999, wall 597 s, p95 task wall 39 s, $0.0414 a task.

- bench_c: 2/2 ($0.0550)
- bench_js: 1/1 ($0.0282)
- bench_maint: 3/3 ($0.1262)
- bench_python: 10/10 ($0.4221)
- bench_shell: 3/3 ($0.1085)
- bench_swift: 1/1 ($0.0476)
- golden: 4/4 ($0.1868)
- multi_turn_recall: 1/1 ($0.0478)
- single_turn: 3/3 ($0.1070)
- subagents: 1/1 ($0.0706)

| task | category | result | cost | wall | attempts | turns | commands |
|---|---|---|---|---|---|---|---|
| g1_recall_multibyte_after_reload | golden | pass | $0.0357 | 27 s | 4 | 2 | 2 |
| g2_subagent_changes_a_file | golden | pass | $0.0448 | 18 s | 5 | 1 | 5 |
| g3_goal_continues_to_done | golden | pass | $0.0546 | 21 s | 3 | 1 | 11 |
| g4_fix_through_long_output | golden | pass | $0.0517 | 17 s | 3 | 1 | 4 |
| a1_fix_mean_off_by_one | single_turn | pass | $0.0255 | 12 s | 2 | 1 | 3 |
| a2_fix_slugify | single_turn | pass | $0.0322 | 36 s | 4 | 1 | 3 |
| a3_add_restock | single_turn | pass | $0.0493 | 29 s | 4 | 1 | 5 |
| b2_fix_then_changelog | multi_turn_recall | pass | $0.0478 | 26 s | 4 | 2 | 6 |
| c2_delegate_two_fixes | subagents | pass | $0.0706 | 19 s | 9 | 1 | 10 |
| k01_csv_to_json | bench_python | pass | $0.0452 | 15 s | 3 | 1 | 3 |
| k02_log_status_counts | bench_python | pass | $0.0590 | 21 s | 3 | 1 | 3 |
| k03_fix_iso_dates | bench_python | pass | $0.0327 | 14 s | 2 | 1 | 4 |
| k04_lru_cache | bench_python | pass | $0.0621 | 20 s | 4 | 1 | 5 |
| k05_rename_function | bench_python | pass | $0.0416 | 18 s | 3 | 1 | 9 |
| k06_cli_json_flag | bench_python | pass | $0.0439 | 14 s | 3 | 1 | 7 |
| k07_fix_tie_order | bench_python | pass | $0.0250 | 19 s | 2 | 1 | 3 |
| k08_roman_numerals | bench_python | pass | $0.0442 | 22 s | 3 | 1 | 5 |
| k09_email_regex | bench_python | pass | $0.0356 | 14 s | 2 | 1 | 3 |
| k10_none_handling | bench_python | pass | $0.0329 | 13 s | 2 | 1 | 3 |
| k11_bash_quoting | bench_shell | pass | $0.0328 | 12 s | 2 | 1 | 3 |
| k12_makefile_targets | bench_shell | pass | $0.0382 | 12 s | 3 | 1 | 9 |
| k13_large_files_script | bench_shell | pass | $0.0375 | 20 s | 2 | 1 | 4 |
| k14_c_reverse | bench_c | pass | $0.0274 | 10 s | 2 | 1 | 4 |
| k15_c_count_words | bench_c | pass | $0.0277 | 11 s | 2 | 1 | 4 |
| k16_swift_fib | bench_swift | pass | $0.0476 | 39 s | 4 | 1 | 6 |
| k17_js_group_by | bench_js | pass | $0.0282 | 15 s | 2 | 1 | 3 |
| k18_version_bump | bench_maint | pass | $0.0404 | 13 s | 3 | 1 | 5 |
| k19_config_migrate | bench_maint | pass | $0.0533 | 19 s | 3 | 1 | 6 |
| k20_dedupe_helper | bench_maint | pass | $0.0325 | 72 s | 5 | 1 | 5 |
