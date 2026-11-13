# 30. Files and device services

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Coder and the other members work on computers. This chapter gives the agent
what only your phone or your browser has: files you choose, the share sheet,
the clipboard, notifications and links, each as an effect the app asks for
and you allow.

## Draft

A device service is an effect with a capability
([Coax Native, "Services and native modules"](../../coax/native.md#services-and-native-modules)).
The app asks to pick a file, share a reply, copy text or show a notice. The
host runs the request only if the app declared the capability and the
operating system allows it. The answer comes back as an action.

You choose what the agent sees. A file reaches the agent only through the
system picker, and the agent gets that file, not a folder or a drive. The
system's own picker, share sheet and permission prompts appear as they are:
the app never draws an imitation of them.

Each service has one owner for its rules. Reading a file into a message uses
the existing text and codec modules. Copying a reply uses the transcript's
Copy. A notification that a long task finished opens the conversation through
the same route as a tap (chapter 4). A capability the device lacks gives an
explicit refusal, never a silent success.

Tests use fakes. A fake picker returns a fixture file and a fake share sheet
records what it was given. The real picker, the share sheet and notification
permission are checked on a device, because a simulator can't prove what the
system shows (chapter 6).

## What we build

A plan for what this chapter ships once the draft is approved:

- Attach a file to a message from the system picker, on the web and the
  iPhone.
- Share a reply and copy it, through the platform's own controls.
- A notification when a turn you left finishes, opening its conversation.

It builds on what exists today:

- [`coax/native.md`](../../coax/native.md)
- [`components/core/text.coax`](../../components/core/text.coax)
- [`apps/shared/host/apple`](../../apps/shared/host/apple)
- [`apps/openagents-ios`](../../apps/openagents-ios)

### The chapter's check

- **Source:** the attach, share and notify actions in the shared app.
- **Trace:** a fake picker returns `notes.md`; the trace prints the
  attachment's name, size and the message part it became.
- **Refusal:** a file larger than the stated limit is refused with the limit
  shown, and nothing is sent.
- **Independent check:** the same file through the web's picker in headless
  Chrome gives the same message part.

Planned paths: none new. Actions go in [`apps/shared`](../../apps/shared);
host effects extend [`apps/shared/host`](../../apps/shared/host).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): the agent on your devices
- **Also:**
  - [Coax Native](../../coax/native.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P3
