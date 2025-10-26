Place the Convex local backend binary here for your platform before building.

Expected filename:
- macOS/Linux: local_backend (chmod +x)
- Windows: local_backend.exe

This binary is the self-hosted Convex local backend (aka convex-local-backend).
We pass the following flags on launch:
- first arg: path to SQLite DB (e.g. ~/.openagents/convex/data.sqlite3)
- --db sqlite
- --interface 127.0.0.1
- --port 3210
- --disable-beacon

CI should download/copy the appropriate release artifact into this directory
so Tauri bundles it as a resource (see tauri.conf.json "bundle.resources").
