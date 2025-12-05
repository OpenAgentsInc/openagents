# 0554 Work Log

- Removed TB run history polling; rely on WebSocket-triggered updates instead.
- Added helpers to map/upsert TB run history entries and update layout on events.
- On TB run start/complete, upsert history immediately and refresh runs on completion.
