# Project Hierarchy

Generated on: 2025-01-21 13:50:01

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
|   |-- ai-slop/
|   |   |-- genesis.md
|   |   |-- old-README.md
|   |   `-- protocols.md
|   |-- episode-transcriptions/
|   |   |-- 001.md
|   |   |-- 095.md
|   |   |-- 138.md
|   |   |-- 139.md
|   |   `-- 140.md
|   |-- chat_template.md
|   |-- configuration.md
|   |-- hierarchy.md
|   |-- htmx-nostr-chat.md
|   |-- newsletter.md
|   |-- prompt.md
|   |-- repomap.md
|   |-- rust-setup.md
|   |-- solver-streaming.md
|   |-- solver.md
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
|   |-- agents/
|   |   |-- agent.rs
|   |   |-- manager.rs
|   |   `-- mod.rs
|   |-- bin/
|   |   `-- deepseek-cli.rs
|   |-- nostr/
|   |   |-- axum_relay.rs
|   |   |-- db.rs
|   |   |-- event.rs
|   |   |-- mod.rs
|   |   `-- subscription.rs
|   |-- server/
|   |   |-- admin/
|   |   |   |-- middleware.rs
|   |   |   |-- mod.rs
|   |   |   `-- routes.rs
|   |   |-- services/
|   |   |   |-- solver/
|   |   |   |   |-- ws/
|   |   |   |   |   |-- files_analysis.rs
|   |   |   |   |   |-- html_formatting.rs
|   |   |   |   |   |-- mod.rs
|   |   |   |   |   |-- solution_generation.rs
|   |   |   |   |   |-- transport.rs
|   |   |   |   |   |-- types.rs
|   |   |   |   |   `-- url_parsing.rs
|   |   |   |   `-- mod.rs
|   |   |   |-- deepseek.rs
|   |   |   |-- github.rs
|   |   |   |-- github_types.rs
|   |   |   |-- mod.rs
|   |   |   |-- openrouter.rs
|   |   |   `-- repomap.rs
|   |   |-- config.rs
|   |   `-- mod.rs
|   |-- configuration.rs
|   |-- database.rs
|   |-- emailoptin.rs
|   |-- filters.rs
|   |-- lib.rs
|   |-- main.rs
|   `-- template_filters.rs
|-- styles/
|   `-- tailwind.css
|-- templates/
|   |-- admin/
|   |   |-- dashboard.html
|   |   `-- login.html
|   |-- components/
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
|   |-- agent/
|   |   |-- core.rs
|   |   |-- manager.rs
|   |   |-- manager_comprehensive.rs
|   |   |-- manager_impl.rs
|   |   |-- mod.rs
|   |   `-- nostr.rs
|   |-- nostr/
|   |   |-- database.rs
|   |   |-- event.rs
|   |   |-- mod.rs
|   |   `-- subscription.rs
|   |-- solver/
|   |   |-- mod.rs
|   |   `-- ws.rs
|   |-- admin_middleware.rs
|   |-- admin_routes.rs
|   |-- agent.rs
|   |-- deepseek.rs
|   |-- emailoptin.rs
|   |-- github.rs
|   |-- health_check.rs
|   |-- openrouter.rs
|   |-- repomap.rs
|   `-- solver.rs
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

29 directories, 130 files
```
