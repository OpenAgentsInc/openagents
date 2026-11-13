-- Security notices as push to the apps (docs/auth/identity-and-methods.md "Notify", #13233). An
-- app whose login is bound to a device key (0153) may give that device a push target: the token
-- Apple or Google gave the app for this install. Each notice (auth.notices) then goes to the
-- account's devices too, beside its emails and keys. What may be kept is decided in
-- auth-device-policy.coax (#authority); these rows hold the facts, and the functions below refuse
-- on their own what the policy must never allow.
--
-- A target is an address for notices and nothing else: it is not a way to log in, it proves
-- nothing, and a notice sent to it carries no link, token or address.

ALTER TABLE auth.devices
  ADD COLUMN push_service text CHECK (push_service IN ('apns', 'apns_sandbox', 'fcm')),
  ADD COLUMN push_token text CHECK (push_token ~ '^[A-Za-z0-9:_-]+$' AND length(push_token) BETWEEN 32 AND 512),
  ADD COLUMN push_at timestamptz,
  ADD CONSTRAINT devices_push_whole CHECK ((push_service IS NULL) = (push_token IS NULL)),
  -- Apple's tokens go to Apple's devices, Google's to Android: never the other way.
  ADD CONSTRAINT devices_push_platform CHECK (push_service IS NULL
    OR (push_service IN ('apns', 'apns_sandbox') AND platform IN ('ios', 'mac'))
    OR (push_service = 'fcm' AND platform = 'android')),
  -- A removed device gets no notices.
  ADD CONSTRAINT devices_push_live CHECK (push_token IS NULL OR revoked_at IS NULL);
-- One install has one token: it is one live device's at a time.
CREATE UNIQUE INDEX devices_push_token ON auth.devices (push_service, push_token) WHERE push_token IS NOT NULL;

-- A device that is removed (by its account, by a recovery, by a merge) forgets its target.
CREATE FUNCTION auth.device_push_forget() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.revoked_at IS NOT NULL THEN NEW.push_service := NULL; NEW.push_token := NULL; NEW.push_at := NULL; END IF;
  RETURN NEW;
END $$;
-- It runs after 0153's device_moved (triggers fire in name order), so a device a merge moved,
-- which that trigger revokes, forgets its target here too.
CREATE TRIGGER device_push_forget BEFORE UPDATE ON auth.devices FOR EACH ROW EXECUTE FUNCTION auth.device_push_forget();

-- What the policy reads about the session that asks: is it an app's, and which platform's device
-- is it bound to ('' when it is bound to none, or its device was removed).
CREATE FUNCTION auth.session_device(acct text, sid text, OUT app boolean, OUT platform text) LANGUAGE sql STABLE AS $$
  SELECT coalesce(bool_or(s.client = 'app'), false), coalesce(max(d.platform), '')
    FROM auth.sessions s LEFT JOIN auth.devices d ON d.id = s.device AND d.revoked_at IS NULL
   WHERE s.id = sid AND s.account = acct
$$;

-- Keeps the push target of the device this session is bound to. The token leaves any other device
-- that held it (the same install, logged in to another account before). The device's id; NULL when
-- the session isn't this account's or is bound to no live device.
CREATE FUNCTION auth.device_push_set(acct text, sid text, service_ text, token_ text) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE dev bigint;
BEGIN
  SELECT d.id INTO dev FROM auth.sessions s JOIN auth.devices d ON d.id = s.device
   WHERE s.id = sid AND s.account = acct AND s.client = 'app' AND d.account = acct AND d.revoked_at IS NULL FOR UPDATE OF d;
  IF dev IS NULL THEN RETURN NULL; END IF;
  UPDATE auth.devices SET push_service = NULL, push_token = NULL, push_at = NULL
   WHERE push_service = service_ AND push_token = token_ AND id <> dev;
  UPDATE auth.devices SET push_service = service_, push_token = token_, push_at = now() WHERE id = dev;
  RETURN dev;
END $$;
-- Forgets the push target of the device this session is bound to: whether there was one.
CREATE FUNCTION auth.device_push_off(acct text, sid text) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  UPDATE auth.devices d SET push_service = NULL, push_token = NULL, push_at = NULL
    FROM auth.sessions s
   WHERE s.id = sid AND s.account = acct AND d.id = s.device AND d.account = acct AND d.push_token IS NOT NULL;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n > 0;
END $$;
-- Where an account's notices go by push: each live device that has a target and still holds a
-- login (a device you logged out of gets nothing).
CREATE FUNCTION auth.push_targets(acct text) RETURNS TABLE (device bigint, platform text, service text, token text) LANGUAGE sql STABLE AS $$
  SELECT d.id, d.platform, d.push_service, d.push_token
    FROM auth.devices d
   WHERE d.account = acct AND d.revoked_at IS NULL AND d.push_token IS NOT NULL
     AND EXISTS (SELECT 1 FROM auth.sessions s WHERE s.device = d.id)
   ORDER BY d.id
$$;
-- Apple or Google said the token is no longer an install's: the device forgets it.
CREATE FUNCTION auth.device_push_gone(dev bigint, token_ text) RETURNS void LANGUAGE sql AS $$
  UPDATE auth.devices SET push_service = NULL, push_token = NULL, push_at = NULL WHERE id = dev AND push_token = token_
$$;
