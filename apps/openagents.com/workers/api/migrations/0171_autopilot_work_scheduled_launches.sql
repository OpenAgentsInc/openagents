-- M6 (#4764): scheduled launches for delegated Autopilot work orders.
-- The scheduled launch record is a typed JSON column holding launchAt,
-- windowMinutes, dispatchedAt, and expiredAt. The scheduled dispatcher is
-- the only release path: placement runs at launch time, not enqueue time.
ALTER TABLE autopilot_work_orders ADD COLUMN scheduled_launch_json TEXT;

CREATE INDEX IF NOT EXISTS idx_autopilot_work_orders_scheduled_launch
  ON autopilot_work_orders (state, created_at)
  WHERE scheduled_launch_json IS NOT NULL;
