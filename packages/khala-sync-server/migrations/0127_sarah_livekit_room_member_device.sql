-- One community room seat belongs to one client at a time.
--
-- The join route mints a deterministic participant ref per (room, member) and
-- upserted a fresh grant onto it unconditionally, so a second client of the
-- same member received a live grant for an identity that was already in the
-- room. Recording which device holds the seat is what makes the difference
-- between a resume by that client and a duplicate participant decidable.
ALTER TABLE sarah_livekit_room_members
  ADD COLUMN IF NOT EXISTS device_ref_digest text CHECK (
    device_ref_digest IS NULL OR device_ref_digest ~ '^[0-9a-f]{64}$'
  );
