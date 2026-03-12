# NIP-28 Chat Channel (First Milestone) — PRD

**Status:** draft  
**Date:** 2026-03-09  
**Implements first milestone of:** [Nostr Group Chat Plan](nostr-group-chat.md)  
**Contract:** [Nostr Managed Chat Contract](../kernel/nostr-managed-chat-contract.md)

---

## Summary

This milestone ships a **basic NIP-28 public chat channel** that desktop users **auto-connect to** with no manual join step. The existing managed chat projection and chat pane already render groups, channels, and messages once data exists; this adds the live relay subscription, ingestion, auto-selection, outbound publish path, and restart persistence needed to make that channel real in the product.

## Goals

- Auto-connect to one default NIP-28 channel.
- Ingest kinds `40`, `41`, and `42` into the existing managed chat projection.
- Auto-select the default channel once the projection has usable content.
- Reuse the current chat pane transcript/composer, including local echo and relay ack/error handling.
- Keep restart persistence on the existing managed chat projection document path.

## Acceptance Criteria

1. A default channel config exists and can be overridden with env vars.
2. Startup subscribes to the channel’s NIP-28 relay events and projects them into managed chat.
3. The chat workspace auto-selects the default channel when data first becomes browseable.
4. The composer publishes kind-42 messages, keeping local echo plus ack/error state.
5. Relay events and outbound message state survive restart through the projection file.

## Notes

- This is intentionally the first milestone only: one default public channel, not full room/server UX.
- The relay/publishing path should reuse the existing managed chat projection rather than inventing a second chat state model.
- Future work can layer richer NIP-29 join/moderation behavior on top of the same projection surface.
