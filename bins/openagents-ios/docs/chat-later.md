# OpenAgents iOS chat: what comes later

Build 6 of the OpenAgents iPhone app draws both chat screens, **Chats** and
**Coder**, with Rust Native's conversation elements: a bottom-anchored
transcript with a jump to the bottom and **Load earlier**, messages by role,
Markdown parsed in Rust, collapsible tool rows, a working row, and a composer
with send and stop. The native design follows the SwiftUI and UIKit rewrite
of the t3code iOS app (pingdotgg/t3code, MIT) and the lessons of Zeron's
UIKit rewrite (zeronsh/comet): Rust decides what to show; the native side
lays out, scrolls, and paints.

This page lists the t3code chat features left out of build 6 on purpose, and
what each needs before it can work. None of them are built.

## Composer

| Feature | What it needs |
| --- | --- |
| Photo attachments: library and camera pickers, drag and drop, an attachment strip, size limits | NIP-HOST `task.create` carries only text. Attachments need an attachment field or an artifact upload the host admits, and a place in the ATIF transcript. |
| Large pastes turned into attachments | The same attachment path. |
| Voice dictation with Apple speech recognition | Only native work and a microphone permission string; no backend change. |
| Model and traits picker | The host's auto-start policy fixes the model today (`coder host autostart --model`). A per-task model needs a `task.create` field the policy admits. |
| Context-usage meter | The host would need to report the engine's context use, for example in activity summaries. |
| Slash commands (`/model` and provider commands) | A command catalog from the host and the operations behind each command. |
| Skills (`$…`) and file mentions (`@…`) | Not wanted for now. |
| Draft persistence across launches | Local only; store the draft in the app's encrypted state. |

## Conversation

| Feature | What it needs |
| --- | --- |
| Follow-ups that continue one task | Today a message in a finished Coder chat starts a new task. Continuing needs a task operation that appends a turn, or engine sessions (NIP-SESS) the phone can drive. |
| Steering a running task | NIP-HOST `task.steer` exists and replaces the task's instructions; the chat composer does not offer it yet, because it is not an ordinary message. |
| Approval and question panels | The engine's approval requests and questions would have to reach the phone, for example as NIP-POL approval requests the host relays, with answers going back through the host. |
| Streaming replies | The Coder tab polls the transcript every few seconds while a task runs. Token streaming needs a live channel from the engine, such as the NIP-REACH direct channel carrying NIP-SESS events. |
| Editing a sent message from its menu | A way to revise a task's prompt; `task.steer` is the closest operation. |
| Inline context links and preview sheets (files, review comments, pull requests) | Structured references in the transcript and read access to what they name. |
| Linked media and remote images in Markdown | Images are shown as their alternative text. Loading them needs an explicit application admission rule; untrusted links stay inert. |
| Reading every record of a very long chat | The phone keeps at most 240 rows of one chat, reads backward in 16 KiB pages, and skips a single record larger than a page. |

## App

| Feature | What it needs |
| --- | --- |
| Push notifications when a task finishes or asks for approval | The push gateway and relay executor that Coder's app uses (see `bins/coder-ios/README.md`), configured for this app's bundle. |
| Share and widget extensions | New app extensions and an app group for shared state. |
