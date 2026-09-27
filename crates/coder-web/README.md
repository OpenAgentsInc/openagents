# Coder web

This Rust server provides a Coder website at `/` and a local task browser at
`/app`. The website draws on Coder's public product and installation guides.
The browser reads the same durable task store and paged ATIF view as
`coder task list` and `coder task view`. It doesn't submit work or grant
execution rights. The private Coder service and its GPUI/Wasm showcase are
reference designs; this implementation uses public contracts and source.

From the monorepo root, start the site and browser:

```sh
cargo run -p coder-web -- --store "$HOME/.openagents/tasks"
```

Open `http://127.0.0.1:4300` for the site or `/app` to inspect your tasks.
The server binds to loopback because the task store contains private prompts,
workspace paths, and trace content. Do not expose the task browser through a
public reverse proxy. A hosted app requires its own session and authorization
boundary. If you have not created a task store, the browser shows an empty
state without creating one.

The web app shows task status, independent checks, paged trace steps, and
artifact references. Reload to read newer task state. It doesn't reproduce
the private service's accounts, billing, live run console, or GPUI canvas.
