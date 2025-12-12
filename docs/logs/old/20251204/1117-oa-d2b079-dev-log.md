# 1117 Work Log (oa-d2b079)

- Starting implementation: add Comment schema (id/text/author/createdAt) with default empty array on tasks.
- Plan updates: Task/TaskCreate default comments [], TaskUpdate allows comments array; export Comment; add schema tests for comments defaults and decoding.
- Implemented Comment schema and added comments arrays to Task/TaskCreate (defaults to []).
- TaskUpdate now accepts comments array; re-exported Comment in tasks index.
- Updated schema tests for comment decoding and defaults.
