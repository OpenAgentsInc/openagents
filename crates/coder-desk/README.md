# `coder-desk`

Both halves of the desk protocol. A Coder program asks this crate what is
on a person's screen and what to change about it, and the crate speaks to
whatever desk is in front of it; a Coder program that is a desk answers the
protocol through the same crate.

The protocol is [`coder_desk::protocol`](src/protocol.rs): the verbs, the
replies, the rows, the socket, and the generation rule. One client exists
because, before it, two Rust callers and fourteen CoderOS scripts each held
their own socket code and their own Hyprland strings.

## The two backends

`Desk::here()` reads what the session announced and picks the backend:

| Announcement | Backend | Speaks |
| --- | --- | --- |
| `CODER_DESK_SOCKET` | `Native` | The desk protocol: one JSON object per line. |
| `CODER_QUEST_SOCKET` | `Native` | CoderQuest's socket, which answers the desk protocol. |
| `HYPRLAND_INSTANCE_SIGNATURE` and `XDG_RUNTIME_DIR` | `Hyprland` | The Hyprland line protocol at `$XDG_RUNTIME_DIR/hypr/<signature>/.socket.sock`. |

A run that carries none is not inside a session, which `Absent::NoSession`
says; a socket nothing answers on is a session that ended, which
`Absent::Unreachable` says. A run reached over SSH and a host with no
compositor carry none.

CoderQuest announces `CODER_DESK_SOCKET` as well, so the first row reaches
it. The `CODER_QUEST_SOCKET` row stays for a pane that an older CoderQuest
started, from before CoderQuest announced `CODER_DESK_SOCKET`.

The Hyprland backend holds every translation: the verb spellings, the
`[workspace N silent]` prefix an `open` carries, the `setprop` dispatches a
`shape` becomes, `keyword monitor` for `scale`, and the regular expression a
`class:` selector becomes. Nothing else in the repository builds one.

## Answering

`serve` is the server half: the socket, the framing, the generation check,
and the dispatch. A desk binds a socket with `serve::bind`, implements
`serve::Desk` over whatever it draws, and answers each `Call` the socket
hands it with `serve::answer`. The socket reads on a thread of its own and
hands each request across a channel, because a desk's state stays on the
desk's thread: the Coder compositor holds Wayland objects and Coder Desktop
holds GPUI views. A desk whose loop sleeps until something happens binds with
`serve::bind_waking` instead, which runs a callback after each request reaches
the channel; the compositor's hardware backend pings its `calloop` loop that
way.

The socket is `$XDG_RUNTIME_DIR/coder-desk/<pid>.sock`, and
`~/.openagents/desk/<pid>.sock` on a host that sets no runtime directory,
which is a Mac. Two desks serve through it in the private Coder repository:
the Coder compositor for the windows on a CoderOS screen, and Coder Desktop
for the panes in its own window. The compositor is planned to move here in
phase 2 of the [CoderOS move](../../docs/os/2026-09-28-coderos-audit.md).
On a CoderOS host that runs Hyprland, the Hyprland backend answers instead.

## Asking

Every verb is an async method on `Desk` and a method with the same name on
`Blocking`, which connects, writes, reads, and closes on the calling thread.
The async half runs the blocking half on the blocking pool, so both send the
same bytes. A caller that runs a frame loop rather than a reactor takes
`Desk::blocking()`.

```rust,ignore
let desk = coder_desk::Desk::here()?;
for window in desk.list().await? {
    println!("{} on desk {}", window.app_id, window.desk);
}
desk.focus(&coder_desk::Selector::Class("coder-child-t1".into())).await?;
```

Each verb answers the protocol's reply, or a `DeskError` that carries the
`Refusal` the desk gave. `DeskError::say` is the sentence a caller shows.

`Desk::reading()` answers the two facts generation 1's `Screen` row does not
carry: which screen has the focus, and the desks with their window counts.

## Testing against it

Add the crate to your dev-dependencies with the `test-support` feature and
use `coder_desk::fake`:

- `fake::Session` holds windows and screens in memory, answers the protocol
  on a socket of its own, and records every request, so a test reads what
  its caller asked and what the desk did about it.
- `fake::hyprland_session` stands up a Hyprland control socket that answers
  each request from a list, which is what a test that asserts the exact
  bytes a verb sends uses.

```rust,ignore
let held = coder_desk::fake::Session::holding(windows, screens);
held.desk().focus(&selector).await?;
assert_eq!(held.requests().len(), 1);
```
