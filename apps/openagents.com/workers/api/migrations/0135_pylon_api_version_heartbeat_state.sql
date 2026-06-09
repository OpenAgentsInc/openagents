ALTER TABLE pylon_api_registrations
  ADD COLUMN client_version TEXT;

ALTER TABLE pylon_api_registrations
  ADD COLUMN client_protocol_version TEXT;

ALTER TABLE pylon_api_registrations
  ADD COLUMN latest_heartbeat_status TEXT;

ALTER TABLE pylon_api_registrations
  ADD COLUMN latest_resource_mode TEXT;

ALTER TABLE pylon_api_registrations
  ADD COLUMN latest_health_refs_json TEXT NOT NULL DEFAULT '[]';

ALTER TABLE pylon_api_registrations
  ADD COLUMN latest_load_refs_json TEXT NOT NULL DEFAULT '[]';

ALTER TABLE pylon_api_registrations
  ADD COLUMN latest_capacity_refs_json TEXT NOT NULL DEFAULT '[]';
