# Project Hierarchy

Generated on: 2025-02-02 21:33:09

### Structure

```
./
|-- assets/
|   |-- fonts/
|   |   |-- BerkeleyMono-Bold.woff
|   |   |-- BerkeleyMono-Bold.woff2
|   |   |-- BerkeleyMono-BoldItalic.woff
|   |   |-- BerkeleyMono-BoldItalic.woff2
|   |   |-- BerkeleyMono-Italic.woff
|   |   |-- BerkeleyMono-Italic.woff2
|   |   |-- BerkeleyMono-Regular.woff
|   |   `-- BerkeleyMono-Regular.woff2
|   |-- favicon.ico
|   |-- fonts.css
|   `-- main.css
|-- configuration/
|   |-- base.yaml
|   |-- local.yaml
|   `-- production.yaml
|-- docs/
|   |-- solve-runs/
|   |   |-- 20250130-1045.md
|   |   |-- 20250130-1105.md
|   |   |-- 20250130-1120.md
|   |   |-- 20250130-1130.md
|   |   |-- 20250130-1145.md
|   |   |-- 20250130-1225.md
|   |   |-- 20250130-1240.md
|   |   |-- 20250131-1830.md
|   |   |-- 20250131-1845.md
|   |   |-- 20250131-1905.md
|   |   |-- 20250131-1915.md
|   |   |-- 20250131-1925.md
|   |   |-- 20250131-1940.md
|   |   |-- 20250131-1945.md
|   |   |-- 20250131-1950.md
|   |   |-- 20250131-2025.md
|   |   |-- 20250131-2050.md
|   |   |-- 20250131-2150.md
|   |   |-- 20250202-1640.md
|   |   |-- 20250202-1720.md
|   |   `-- 20250202-1730.md
|   |-- adding-tools.md
|   |-- chat_template.md
|   |-- configuration.md
|   |-- hierarchy.md
|   |-- local-repos.md
|   |-- repomap.md
|   |-- repomap_generation.md
|   |-- solver.md
|   |-- templates.md
|   `-- test-failures-analysis.md
|-- migrations/
|   |-- 20240126000000_create_chat_tables.sql
|   |-- 20250110000000_initial.sql
|   `-- 20250126023641_create_users_table.sql
|-- scripts/
|   |-- generate_hierarchy.sh*
|   |-- init_db.sh*
|   `-- init_redis.sh*
|-- src/
|   |-- bin/
|   |   |-- solver_impl/
|   |   |   |-- changes.rs
|   |   |   |-- changes_analysis.rs
|   |   |   |-- context.rs
|   |   |   |-- files.rs
|   |   |   |-- issue.rs
|   |   |   |-- mod.rs
|   |   |   |-- pre_analysis.rs
|   |   |   `-- types.rs
|   |   |-- chat.rs
|   |   |-- deepseek-cli.rs
|   |   |-- generate-repomap.rs
|   |   |-- repo.rs
|   |   |-- solver.rs
|   |   `-- solver_orig.rs
|   |-- repo/
|   |   |-- analysis.rs
|   |   |-- git.rs
|   |   |-- mod.rs
|   |   |-- test.rs
|   |   `-- types.rs
|   |-- server/
|   |   |-- handlers/
|   |   |   |-- auth.rs
|   |   |   |-- mod.rs
|   |   |   `-- user.rs
|   |   |-- models/
|   |   |   |-- chat.rs
|   |   |   |-- mod.rs
|   |   |   `-- user.rs
|   |   |-- services/
|   |   |   |-- deepseek/
|   |   |   |   |-- methods/
|   |   |   |   |   |-- chat.rs
|   |   |   |   |   |-- chat_stream.rs
|   |   |   |   |   |-- chat_with_tool_response.rs
|   |   |   |   |   |-- chat_with_tools.rs
|   |   |   |   |   `-- mod.rs
|   |   |   |   |-- mod.rs
|   |   |   |   |-- service.rs
|   |   |   |   |-- streaming.rs
|   |   |   |   `-- types.rs
|   |   |   |-- gateway/
|   |   |   |   |-- mod.rs
|   |   |   |   |-- streaming.rs
|   |   |   |   `-- types.rs
|   |   |   |-- github_issue/
|   |   |   |   |-- conversions.rs
|   |   |   |   `-- mod.rs
|   |   |   |-- ollama/
|   |   |   |   |-- config.rs
|   |   |   |   |-- mod.rs
|   |   |   |   |-- service.rs
|   |   |   |   `-- types.rs
|   |   |   |-- openrouter/
|   |   |   |   |-- mod.rs
|   |   |   |   |-- service.rs
|   |   |   |   `-- types.rs
|   |   |   |-- auth.rs
|   |   |   |-- chat_database.rs
|   |   |   |-- github_types.rs
|   |   |   |-- mod.rs
|   |   |   |-- model_router.rs
|   |   |   `-- repomap.rs
|   |   |-- ws/
|   |   |   |-- handlers/
|   |   |   |   |-- chat.rs
|   |   |   |   `-- mod.rs
|   |   |   |-- mod.rs
|   |   |   |-- transport.rs
|   |   |   `-- types.rs
|   |   |-- config.rs
|   |   |-- mod.rs
|   |   `-- tools.rs
|   |-- solver/
|   |   |-- changes/
|   |   |   |-- apply.rs
|   |   |   |-- generation.rs
|   |   |   |-- mod.rs
|   |   |   |-- parsing.rs
|   |   |   |-- tests.rs
|   |   |   `-- types.rs
|   |   |-- cli.rs
|   |   |-- config.rs
|   |   |-- context.rs
|   |   |-- display.rs
|   |   |-- file_list.rs
|   |   |-- github.rs
|   |   |-- json.rs
|   |   |-- mod.rs
|   |   |-- planning.rs
|   |   |-- solution.rs
|   |   |-- state.rs
|   |   |-- streaming.rs
|   |   `-- types.rs
|   |-- configuration.rs
|   |-- database.rs
|   |-- filters.rs
|   |-- lib.rs
|   |-- main.rs
|   |-- repomap.rs
|   `-- routes.rs
|-- styles/
|   `-- tailwind.css
|-- templates/
|   |-- admin/
|   |   |-- dashboard.html
|   |   `-- login.html
|   |-- components/
|   |   |-- auth/
|   |   |   |-- auth_scripts.html
|   |   |   |-- error.html
|   |   |   `-- error_handler.js
|   |   |-- chat/
|   |   |   |-- error_section.html
|   |   |   |-- head_scripts.html
|   |   |   |-- header.html
|   |   |   |-- login_overlay.html
|   |   |   |-- main_chat.html
|   |   |   |-- sidebar_left.html
|   |   |   |-- sidebar_right.html
|   |   |   |-- templates.html
|   |   |   `-- websocket_scripts.html
|   |   |-- features.html
|   |   `-- hero.html
|   |-- layouts/
|   |   |-- auth_base.html
|   |   |-- base.html
|   |   |-- chat_base.html
|   |   |-- chat_content.html
|   |   `-- content.html
|   |-- macros/
|   |   |-- blog.html
|   |   |-- blog_post.html
|   |   |-- nav.html
|   |   |-- ui.html
|   |   `-- video.html
|   |-- pages/
|   |   |-- 404.html
|   |   |-- chat.html
|   |   |-- coming-soon.html
|   |   |-- company.html
|   |   |-- home.html
|   |   |-- login.html
|   |   |-- onyx.html
|   |   |-- repomap.html
|   |   |-- services.html
|   |   |-- signup.html
|   |   |-- solver.html
|   |   `-- video-series.html
|   |-- blog_post.html
|   `-- header.html
|-- tests/
|   |-- common/
|   |   `-- mod.rs
|   |-- model_router_service/
|   |   |-- chat.rs
|   |   |-- mod.rs
|   |   |-- routing.rs
|   |   `-- tool_execution.rs
|   |-- auth_error_handling.rs
|   |-- auth_pages.rs
|   |-- auth_signup_test.rs
|   |-- chat_database.rs
|   |-- chat_router_integration.rs
|   |-- deepseek.rs
|   |-- gateway.rs
|   |-- health_check.rs
|   |-- mod.rs
|   |-- model_router.rs
|   |-- oidc_client.rs
|   |-- oidc_signup.rs
|   |-- ollama.rs
|   |-- repomap.rs
|   |-- signup_flow.rs
|   |-- solver.rs
|   |-- solver_changes.rs
|   |-- solver_context.rs
|   |-- solver_file_list.rs
|   |-- solver_loop.rs
|   |-- solver_ollama.rs
|   |-- solver_state.rs
|   |-- solver_types.rs
|   |-- tool_selection.rs
|   `-- user.rs
|-- Cargo.lock
|-- Cargo.toml
|-- DEVELOPMENT.md
|-- Dockerfile
|-- README.md
|-- build.rs
|-- package.json
|-- pnpm-lock.yaml
|-- postcss.config.js
|-- spec.yaml
`-- tailwind.config.cjs

38 directories, 219 files
```
