# Jira Tickets — NIP-28 Chat Usability (Milestone 2)

Use this as the issue breakdown for the second NIP-28 chat milestone.

**PRD:** [nip28-chat-channel-prd.md](nip28-chat-channel-prd.md) (first milestone) — this milestone builds on top of that.

---

## Epic

- **Title:** `NIP-28: Chat usability — display names, channel name, Discord-style layout`
- **Goal:** Make the chat channel readable and feel like a real chat product. Channel name renders correctly, messages show author display names (kind-0), OA presence events are visually separated, and each message has a Discord-style header with timestamp.

---

## Ticket F — Fix channel name display

**Summary:** Sidebar and header show raw truncated channel ID instead of channel name.

**Problem:** `managed_channel_label()` falls back to `channel.channel_id` because `channel.metadata.name` is empty after ingestion. Root cause: `ManagedChannelCreateEvent::from_event()` is likely failing to parse the standard NIP-28 kind-40 payload on the relay (OA custom format vs. plain NIP-28 format mismatch).

**Acceptance criteria:**
- Sidebar channel label shows the channel name from kind-40/41, not the event ID.
- If no kind-40 has arrived yet, fall back to truncated ID (existing behavior).

**Files:**
- `apps/autopilot-desktop/src/app_state/chat_projection.rs` — kind-40/41 handler (~lines 843–862)
- `apps/autopilot-desktop/src/panes/chat.rs` — `managed_channel_label()` (~lines 1106–1114)
- `crates/nostr/core/src/nip28.rs` — `ChannelMetadata` struct

**Notes:**
- Inspect the actual kind-40 event arriving from the relay and compare to `ManagedChannelCreateEvent::from_event()` expected format.
- Fix the parser, or add a fallback: if `ManagedChannelCreateEvent` parse fails, try parsing content as plain `ChannelMetadata` JSON.

---

## Ticket G — Introduce dedicated human chat channel (separate from presence channel)

**Summary:** Split the single NIP-28 channel into two: one for OA autopilot presence (existing), one for clean human chat (new).

**Problem:** `OA_DEFAULT_NIP28_CHANNEL_ID` (the existing channel) is used by both autopilot nodes publishing presence payloads and by humans posting chat messages. These concerns don't belong in the same channel. Content-filtering in the UI is a workaround; the right fix is channel separation at the source.

**Decision:**
- `OA_DEFAULT_NIP28_CHANNEL_ID` (existing `ebf2e35…`) → **presence/system channel**. Autopilot nodes continue publishing `oa.autopilot.presence.v1` kind-42 events here. No change to the publish path.
- `OA_DEFAULT_NIP28_CHAT_CHANNEL_ID` (new) → **human chat channel**. This is what the composer targets and what the transcript displays.

**Acceptance criteria:**
- New env var `OA_DEFAULT_NIP28_CHAT_CHANNEL_ID` added with a new default channel ID.
- `DefaultNip28ChannelConfig` gains a `chat_channel_id` field alongside the existing `channel_id` (presence).
- `nip28_chat_lane` subscribes to `chat_channel_id` for the transcript (kinds 40, 41, 42).
- Composer sends outbound kind-42 messages to `chat_channel_id`, not the presence channel.
- The presence channel (`channel_id`) is not subscribed by the chat lane — fully separated for now.
- Chat transcript contains only human messages. No presence JSON.

**Prerequisite:**
- A new NIP-28 channel must be created (kind-40 published to the relay) to obtain the default `chat_channel_id` value.

**Files:**
- `apps/autopilot-desktop/src/app_state.rs` — `DefaultNip28ChannelConfig`: add `chat_channel_id` field, new env var constant `OA_DEFAULT_NIP28_CHAT_CHANNEL_ID`, default value
- `apps/autopilot-desktop/src/nip28_chat_lane.rs` — use `config.chat_channel_id` for subscription filters and outbound publish
- `apps/autopilot-desktop/src/app_state/chat_projection.rs` — no change needed (lane feeds the same projection)

**Notes:**
- Presence channel subscription can be reintroduced later (separate lane or config flag) to power a "who's online" indicator without polluting the transcript.
- The existing `OA_DEFAULT_NIP28_CHANNEL_ID` env var and default value are unchanged — presence channel continues to work as-is.

---

## Ticket H — Kind-0 user metadata fetch lane

**Summary:** No kind-0 (NIP-01 user metadata) infrastructure exists. Build the fetch lane and cache.

**Problem:** Author pubkeys in messages cannot be resolved to display names because there is no mechanism to fetch or store kind-0 events.

**Acceptance criteria:**
- Author pubkeys seen in incoming kind-42 events are queued for kind-0 fetch.
- Kind-0 events are fetched from the relay and stored in a per-session cache.
- Cache entries are keyed by pubkey hex and hold: `display_name`, `name`, `picture`, `fetched_at`.
- Pubkeys already in cache are not re-fetched within the same session.

**Files:**
- `apps/autopilot-desktop/src/nip28_chat_lane.rs` — reference pattern for the new lane
- `apps/autopilot-desktop/src/app_state.rs` — add `user_metadata_cache: UserMetadataCache` to `RenderState`
- `apps/autopilot-desktop/src/render.rs` — spawn `Kind0FetchLaneWorker` at startup
- `apps/autopilot-desktop/src/input/reducers/mod.rs` — drain kind-0 updates into cache; enqueue new pubkeys on kind-42 ingestion

**Notes:**
- Model the lane after `nip28_chat_lane.rs`: background thread, tokio runtime, mpsc channels.
- Subscribe filter: `{"kinds": [0], "authors": [<batch of pubkeys>]}`.
- Batch pubkey requests to avoid one subscription per message.
- `UserMetadata` struct: `display_name: Option<String>, name: Option<String>, picture: Option<String>`.

---

## Ticket I — Render author display name in message rows

**Summary:** Message author rows show truncated pubkey; replace with resolved display name when available.

**Problem:** `managed_message_role_label()` always calls `compact_hex_label(&message.author_pubkey, 8)`. No name resolution happens even when a kind-0 is available.

**Acceptance criteria:**
- When kind-0 is cached for an author: show `display_name` (fallback to `name`) truncated to 20 chars.
- When no kind-0 is cached: show existing `pubkey[0..8]` truncation (no regression).
- Local user: keep existing "you" label.
- Same resolution applies to DM message rows.

**Files:**
- `apps/autopilot-desktop/src/panes/chat.rs` — `managed_message_role_label()` (~lines 1174–1189)
- `apps/autopilot-desktop/src/panes/chat.rs` — `direct_message_role_label()` (~lines 1333–1357)

**Notes:**
- Depends on Ticket H (cache must exist before this can render names).
- Thread `&UserMetadataCache` into the message rendering context.

---

## Ticket J — Discord-style message layout with timestamps

**Summary:** Add per-message timestamps and a Discord-style author + timestamp header row.

**Problem:** Current layout is `[#1] [ebf2e350…] [acked]` — no timestamp, no clear visual hierarchy. Not readable as a chat product.

**Acceptance criteria:**
- Each message shows: `[#N]  display_name  HH:MM` on the header line (index dim, name accented, time dim).
- Same-day messages show `HH:MM`; older messages show `MMM D, HH:MM`.
- Message content is indented to visually align below the name (not the index).
- Delivery state indicator (`[sending]`, `[failed]`) moves to end of content, not on the header line.

**Files:**
- `apps/autopilot-desktop/src/panes/chat.rs` — message rendering loop (~lines 3274–3332); add `format_message_timestamp()` helper
- `apps/autopilot-desktop/src/panes/chat.rs` — `managed_message_role_label()` (~lines 1174–1189); restructure into separate header components

**Notes:**
- Use `std::time::SystemTime` / `UNIX_EPOCH` for current time comparison against `created_at`.
- Keep the `[#N]` index prefix for the `reply <#N>` composer workflow to remain functional.
- Depends on Ticket I for display names, but timestamps can be added independently.

---

## Implementation Order

| Order | Ticket | Depends on |
|-------|--------|------------|
| 1 | F — Channel name | — |
| 2 | G — Filter presence noise | — |
| 3 | H — Kind-0 fetch lane | — |
| 4 | I — Display names | H |
| 5 | J — Discord layout + timestamps | I (names), can land partially without it |

F and G are independent quick wins. H → I → J is the display name chain.
