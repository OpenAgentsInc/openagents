# Project Hierarchy

Generated on: 2025-01-24 10:54:53

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
|   `-- templates.md
|-- migrations/
|   |-- 20250110000000_initial.sql
|   |-- 20250112001624_create_subscriptions_table.sql
|   `-- 20250112002000_create_agent_tables.sql
|-- scripts/
|   |-- generate_hierarchy.sh*
|   |-- init_db.sh*
|   `-- init_redis.sh*
|-- src/
|   |-- bin/
|   |   |-- deepseek-cli.rs
|   |   `-- repo.rs
|   |-- repo/
|   |   |-- analysis.rs
|   |   |-- git.rs
|   |   |-- mod.rs
|   |   |-- test.rs
|   |   `-- types.rs
|   |-- server/
|   |   |-- services/
|   |   |   |-- deepseek/
|   |   |   |   |-- mod.rs
|   |   |   |   |-- service.rs
|   |   |   |   |-- streaming.rs
|   |   |   |   `-- types.rs
|   |   |   |-- github_issue.rs
|   |   |   |-- github_types.rs
|   |   |   |-- mod.rs
|   |   |   `-- repomap.rs
|   |   |-- ws/
|   |   |   |-- handlers/
|   |   |   |   |-- chat.rs
|   |   |   |   `-- mod.rs
|   |   |   |-- mod.rs
|   |   |   |-- transport.rs
|   |   |   `-- types.rs
|   |   |-- config.rs
|   |   `-- mod.rs
|   |-- configuration.rs
|   |-- database.rs
|   |-- filters.rs
|   |-- lib.rs
|   |-- main.rs
|   `-- repomap.rs
|-- styles/
|   `-- tailwind.css
|-- templates/
|   |-- admin/
|   |   |-- dashboard.html
|   |   `-- login.html
|   |-- components/
|   |   |-- chat/
|   |   |   |-- error_section.html
|   |   |   |-- head_scripts.html
|   |   |   |-- header.html
|   |   |   |-- main_chat.html
|   |   |   |-- sidebar_left.html
|   |   |   |-- sidebar_right.html
|   |   |   |-- templates.html
|   |   |   `-- websocket_scripts.html
|   |   |-- features.html
|   |   `-- hero.html
|   |-- layouts/
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
|   |   |-- onyx.html
|   |   |-- repomap.html
|   |   |-- services.html
|   |   |-- solver.html
|   |   `-- video-series.html
|   `-- header.html
|-- tests/
|   |-- deepseek.rs
|   |-- health_check.rs
|   |-- repomap.rs
|   `-- tool_selection.rs
|-- Cargo.lock
|-- Cargo.toml
|-- DEVELOPMENT.md
|-- Dockerfile
|-- README.md
|-- package.json
|-- pnpm-lock.yaml
|-- postcss.config.js
|-- spec.yaml
`-- tailwind.config.cjs

24 directories, 102 files
```
