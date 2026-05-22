This is `rusqlite` 0.31.0 with one Cargo metadata patch:

- `libsqlite3-sys` is raised from `0.28.0` to `0.30.1`.

The patch lets the retained Spark SDK dependency and the new Pylon `ldk-node`
runtime share one `links = "sqlite3"` crate in the workspace dependency graph.
No Rust source files are changed.
