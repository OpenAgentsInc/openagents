# Waiting for the next Coder release

Changes that only reach people through a new Coder binary. While the binary
is frozen at 1.0.0 ([1.0-freeze.md](1.0-freeze.md)) they wait here. Each
entry names its issue, which stays open with a comment saying it waits for
this list.

| Change | Issue | Why it needs a new binary |
| --- | --- | --- |
| Hand a user's Vertex credential to Claude Code through an in-memory file, never `/tmp` | #11771 | The launch script that writes the file is built by the user's own `coder` (`coder remote delegate --on boat\|gce`) |
| Call the Decisions API at `https://api.openagents.com/v1/decisions` (`jev::decisions::OPENAGENTS` is `https://openagents.com/api` in 1.0.0) | — | The address is compiled into `coder`; the old one keeps working meanwhile |
