# Project Hierarchy

Generated on: 2025-01-14 08:52:59

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
|   |-- configuration.md
|   |-- hierarchy.md
|   |-- htmx-nostr-chat.md
|   |-- newsletter.md
|   |-- prompt.md
|   `-- rust-setup.md
|-- migrations/
|   |-- 20250110000000_initial.sql
|   |-- 20250112001624_create_subscriptions_table.sql
|   `-- 20250112002000_create_agent_tables.sql
|-- scripts/
|   `-- generate_hierarchy.sh*
|-- src/
|   |-- agents/
|   |   |-- agent.rs
|   |   |-- manager.rs
|   |   `-- mod.rs
|   |-- nostr/
|   |   |-- db.rs
|   |   |-- event.rs
|   |   |-- mod.rs
|   |   |-- relay.rs
|   |   `-- subscription.rs
|   |-- server/
|   |   |-- admin/
|   |   |   |-- middleware.rs
|   |   |   |-- mod.rs
|   |   |   `-- routes.rs
|   |   |-- config.rs
|   |   |-- mod.rs
|   |   `-- routes.rs
|   |-- configuration.rs
|   |-- database.rs
|   |-- emailoptin.rs
|   |-- lib.rs
|   `-- main.rs
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
|   |   `-- content.html
|   |-- macros/
|   |   |-- blog.html
|   |   |-- nav.html
|   |   `-- video.html
|   |-- pages/
|   |   |-- 404.html
|   |   |-- coming-soon.html
|   |   |-- company.html
|   |   |-- home.html
|   |   |-- mobile-app.html
|   |   |-- services.html
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
|   |-- admin_middleware.rs
|   |-- admin_routes.rs
|   |-- agent.rs
|   |-- emailoptin.rs
|   `-- health_check.rs
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

24 directories, 94 files
```
