-- CFG-4 Domain 4 (#8519, epic #8515): Khala Mobile push tables HARD cutover —
-- `push_device_tokens` and `push_notification_preferences` become Cloud SQL
-- Postgres-AUTHORITATIVE (Worker D1 migrations 0304/0305). The D1 code path
-- for these two tables is DELETED — no dual-write, no mirror, no read flag.
--
-- Unlike every other CFG-4 domain, these two tables had NO Postgres twin and
-- NO dual-write mirror before this migration: the mobile push MVP shipped D1
-- rows only, and no traffic has registered a device yet (both prod D1 tables
-- are empty at cutover time). So this migration CREATES the twins fresh as
-- the sole authority; the one-time coordinator copy is a no-op for empty
-- source tables (see docs/khala-sync/RUNBOOK.md — "Mobile push tables HARD
-- cutover (CFG-4)").
--
-- CFG-3 (commit 40ae3aa6d5) already hard-cut the REST of mobile session
-- support (AUTH_STORAGE KV + openauth_storage) onto the Postgres KvStore;
-- the "push-token prune keys" that live in that KV are UNAFFECTED here — only
-- the two D1 tables move.
--
-- TYPE FIDELITY (the 0015/0028 twin rules): TEXT ISO-8601 timestamps stay
-- text (sort correctly, hash byte-exact); a D1 INTEGER 0/1 flag stays
-- smallint with a CHECK (the 0015 "0/1 booleans stay smallint" convention),
-- NOT bigint. Every D1 UNIQUE/PK/CHECK is ported byte-for-byte because the
-- Worker now writes these tables here directly and its upserts
-- (ON CONFLICT (user_id, device_id) / ON CONFLICT (user_id)) require the
-- matching arbiters to exist in Postgres.
--
-- INDEXES ARE RE-DERIVED FROM THE ACTUAL READ PATHS (the KS-8.2 rule), from
-- push/push-device-tokens.ts:
--   * listPushDeviceTokensForUser: WHERE user_id = ? ORDER BY updated_at DESC
--     — the (user_id, updated_at DESC) accelerator (mirrors D1 0304).
--   * sendExpoPushMessages invalidation + removePushDeviceTokensByExpoToken:
--     WHERE expo_push_token = ? — the (expo_push_token) accelerator
--     (mirrors D1 0304). The primary-key (user_id, device_id) covers the
--     register/unregister/active-list reads; nothing to add there.
--   * push_notification_preferences is read/written by primary key only
--     (readPushNotificationPreference / writePushNotificationPreference,
--     WHERE user_id = ?) — the PK is the only index it needs.
--
-- NO FOREIGN KEYS (same posture as every other khala-sync twin; a push row
-- referencing a since-deleted user simply never matches a live read).

-- --------------------------------------------------------------------------
-- push_device_tokens (Worker migration 0304)
-- --------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS push_device_tokens (
  user_id                     text NOT NULL,
  device_id                   text NOT NULL,
  expo_push_token             text NOT NULL,
  platform                    text NOT NULL CHECK (platform IN ('ios', 'android')),
  access_token_revocation_key text,
  created_at                  text NOT NULL,
  updated_at                  text NOT NULL,
  PRIMARY KEY (user_id, device_id)
);

CREATE INDEX IF NOT EXISTS push_device_tokens_user_idx
  ON push_device_tokens (user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS push_device_tokens_expo_token_idx
  ON push_device_tokens (expo_push_token);

-- --------------------------------------------------------------------------
-- push_notification_preferences (Worker migration 0305)
-- --------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS push_notification_preferences (
  user_id      text NOT NULL,
  push_enabled smallint NOT NULL DEFAULT 1 CHECK (push_enabled IN (0, 1)),
  created_at   text NOT NULL,
  updated_at   text NOT NULL,
  PRIMARY KEY (user_id)
);
