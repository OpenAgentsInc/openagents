ALTER TABLE thread_files
  ADD COLUMN download_enabled INTEGER NOT NULL DEFAULT 1
    CHECK (download_enabled IN (0, 1));

CREATE TABLE thread_file_message_refs (
  id TEXT PRIMARY KEY,
  file_id TEXT NOT NULL,
  team_id TEXT,
  thread_id TEXT NOT NULL,
  message_id TEXT NOT NULL,
  reference_kind TEXT NOT NULL
    CHECK (
      reference_kind IN (
        'message_attachment',
        'autopilot_input',
        'autopilot_answer'
      )
    ),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  UNIQUE(file_id, message_id, reference_kind),
  FOREIGN KEY (file_id) REFERENCES thread_files(id),
  FOREIGN KEY (team_id) REFERENCES teams(id),
  FOREIGN KEY (message_id) REFERENCES team_chat_messages(id)
);

CREATE INDEX thread_file_message_refs_file_created_idx
  ON thread_file_message_refs(file_id, created_at)
  WHERE deleted_at IS NULL;

CREATE INDEX thread_file_message_refs_message_idx
  ON thread_file_message_refs(message_id)
  WHERE deleted_at IS NULL;

INSERT OR IGNORE INTO thread_file_message_refs
  (id, file_id, team_id, thread_id, message_id, reference_kind, created_at, updated_at)
SELECT
  'thread_file_message_ref_' || lower(hex(randomblob(16))),
  selected_file_ids.value,
  messages.team_id,
  COALESCE(messages.autopilot_thread_id, 'team:' || messages.team_id || ':chat'),
  messages.id,
  CASE
    WHEN messages.kind = 'system' THEN 'autopilot_answer'
    WHEN messages.kind = 'autopilot_intent' THEN 'autopilot_input'
    ELSE 'message_attachment'
  END,
  messages.created_at,
  messages.updated_at
FROM team_chat_messages AS messages
JOIN json_each(
  COALESCE(
    json_extract(messages.metadata_json, '$.selectedTeamFileIds'),
    json_extract(messages.metadata_json, '$.context.selectedTeamFileIds'),
    '[]'
  )
) AS selected_file_ids
JOIN thread_files AS files
  ON files.id = selected_file_ids.value
 AND files.team_id = messages.team_id
 AND files.deleted_at IS NULL
WHERE messages.deleted_at IS NULL;
