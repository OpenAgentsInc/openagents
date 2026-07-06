-- MM-G1 (#8485): Khala Mobile push notification device-token registry.
--
-- Keyed per (user_id, device_id) so re-registering the SAME device (e.g. a
-- refreshed Expo push token after a reinstall) upserts rather than
-- duplicating rows. `access_token_revocation_key` stores the SAME KV lookup
-- key `mobile-session.ts`'s `revokeMobileAccessToken`/`isMobileAccessTokenRevoked`
-- already use (a SHA-256 hash of the bearer access token active at
-- registration time, never the raw token itself) — see
-- `push/push-device-tokens.ts`'s `pruneRevokedPushDeviceTokens` for how this
-- is used to prune a row once that access token is explicitly revoked
-- (sign-out), without this table ever storing a raw bearer token at rest.

CREATE TABLE IF NOT EXISTS push_device_tokens (
  user_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  expo_push_token TEXT NOT NULL,
  platform TEXT NOT NULL CHECK (platform IN ('ios', 'android')),
  access_token_revocation_key TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, device_id)
);

CREATE INDEX IF NOT EXISTS idx_push_device_tokens_user
  ON push_device_tokens (user_id, updated_at DESC);

-- Supports the sender's "one row per distinct expo push token" fan-out and
-- the push-service invalidation-receipt lookup (MM-G2, #8486): given an
-- Expo push token a delivery receipt reports as invalid, find every row
-- that currently carries it, regardless of which user/device.
CREATE INDEX IF NOT EXISTS idx_push_device_tokens_expo_token
  ON push_device_tokens (expo_push_token);
