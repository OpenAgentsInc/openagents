# Project Hierarchy

Generated on: 2025-01-27 23:34:02

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
|   |-- adding-tools.md
|   |-- chat_template.md
|   |-- configuration.md
|   |-- hierarchy.md
|   |-- local-repos.md
|   |-- repomap.md
|   |-- repomap_generation.md
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
|   |   |-- chat.rs
|   |   |-- deepseek-cli.rs
|   |   |-- generate-repomap.rs
|   |   `-- repo.rs
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
|   |   |   |-- auth.rs
|   |   |   |-- chat_database.rs
|   |   |   |-- github_issue.rs
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
|   |-- health_check.rs
|   |-- mod.rs
|   |-- model_router.rs
|   |-- oidc_client.rs
|   |-- oidc_signup.rs
|   |-- repomap.rs
|   |-- signup_flow.rs
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

30 directories, 147 files
```
