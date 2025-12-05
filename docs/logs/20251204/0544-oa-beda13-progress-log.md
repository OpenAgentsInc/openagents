# 0544 Work Log (oa-beda13 dev)

- Added TB run history push message handling (tb_run_history) in hud protocol and mainview; UI now applies run history directly from WebSocket events and prunes stale details.
- Desktop server now broadcasts latest run history on UI connect and when TB runs complete, eliminating periodic polling.
- Removed TB run history polling interval; keep single initial fetch after socket connect as fallback.

