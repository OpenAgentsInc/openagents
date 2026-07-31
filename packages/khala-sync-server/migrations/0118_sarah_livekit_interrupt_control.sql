ALTER TABLE sarah_livekit_room_bindings
  ADD COLUMN IF NOT EXISTS worker_interrupt_sequence bigint NOT NULL DEFAULT 0
    CHECK (worker_interrupt_sequence >= 0),
  ADD COLUMN IF NOT EXISTS worker_interrupt_requested_at text;
