CREATE TABLE IF NOT EXISTS order_triage_records (
  id TEXT PRIMARY KEY NOT NULL,
  software_order_id TEXT NOT NULL REFERENCES software_orders(id) ON DELETE CASCADE,
  classification TEXT NOT NULL CHECK (
    classification IN (
      'runnable_site',
      'runnable_general_autopilot',
      'needs_clarification',
      'smoke_or_test',
      'legal_sensitive_policy_review',
      'unavailable_or_declined'
    )
  ),
  operator_priority INTEGER NOT NULL DEFAULT 100,
  first_batch_eligible INTEGER NOT NULL DEFAULT 0 CHECK (
    first_batch_eligible IN (0, 1)
  ),
  hold_reason TEXT,
  next_action TEXT NOT NULL,
  customer_safe_status TEXT NOT NULL,
  customer_safe_summary TEXT NOT NULL,
  reviewer_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  reviewed_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS order_triage_records_active_order_idx
  ON order_triage_records(software_order_id)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS order_triage_records_priority_idx
  ON order_triage_records(first_batch_eligible DESC, operator_priority ASC, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS order_triage_records_classification_idx
  ON order_triage_records(classification, updated_at DESC)
  WHERE archived_at IS NULL;

INSERT OR IGNORE INTO order_triage_records
  (id,
   software_order_id,
   classification,
   operator_priority,
   first_batch_eligible,
   hold_reason,
   next_action,
   customer_safe_status,
   customer_safe_summary,
   reviewer_user_id,
   reviewed_at,
   created_at,
   updated_at)
SELECT 'order_triage_chefgroep_site_remake',
       id,
       'runnable_site',
       10,
       1,
       NULL,
       'Run existing-project compatibility check, create a Site assignment, then prepare the first saved version for review.',
       'scoping',
       'OpenAgents is preparing this website order for the first overnight Sites batch.',
       NULL,
       datetime('now'),
       datetime('now'),
       datetime('now')
  FROM software_orders
 WHERE id = 'software_order_57593c2c60c54d25a140588633e3b318';

INSERT OR IGNORE INTO order_triage_records
  (id,
   software_order_id,
   classification,
   operator_priority,
   first_batch_eligible,
   hold_reason,
   next_action,
   customer_safe_status,
   customer_safe_summary,
   reviewer_user_id,
   reviewed_at,
   created_at,
   updated_at)
SELECT 'order_triage_ben_otec_site',
       id,
       'runnable_site',
       20,
       1,
       NULL,
       'Create a Site assignment for the OTEC website and prepare public proof closeout after review.',
       'scoping',
       'OpenAgents is preparing this website order for the first overnight Sites batch.',
       NULL,
       datetime('now'),
       datetime('now'),
       datetime('now')
  FROM software_orders
 WHERE id IN (
   'software_order_c34f3a52d60b41d699b71525365b6ee5',
   'software_order_c34f3a52d60b41d699b71525365c2e5'
 );

INSERT OR IGNORE INTO order_triage_records
  (id,
   software_order_id,
   classification,
   operator_priority,
   first_batch_eligible,
   hold_reason,
   next_action,
   customer_safe_status,
   customer_safe_summary,
   reviewer_user_id,
   reviewed_at,
   created_at,
   updated_at)
SELECT 'order_triage_openagents_marketing',
       id,
       'needs_clarification',
       30,
       0,
       'Needs a scoped first slice before launch.',
       'Define the first marketing slice, then create a general Autopilot or Sites-adjacent assignment.',
       'needs_scope',
       'OpenAgents is scoping the first useful marketing slice before launch.',
       NULL,
       datetime('now'),
       datetime('now'),
       datetime('now')
  FROM software_orders
 WHERE id = 'software_order_backfill_33615693';

INSERT OR IGNORE INTO order_triage_records
  (id,
   software_order_id,
   classification,
   operator_priority,
   first_batch_eligible,
   hold_reason,
   next_action,
   customer_safe_status,
   customer_safe_summary,
   reviewer_user_id,
   reviewed_at,
   created_at,
   updated_at)
SELECT 'order_triage_uplink_smoke',
       id,
       'smoke_or_test',
       200,
       0,
       'Testing order; not promoted to customer overnight work.',
       'Hold until an operator promotes this smoke order into a real scope.',
       'held',
       'This request is held while OpenAgents confirms whether it is a real task or a test.',
       NULL,
       datetime('now'),
       datetime('now'),
       datetime('now')
  FROM software_orders
 WHERE id = 'software_order_backfill_100535789';

INSERT OR IGNORE INTO order_triage_records
  (id,
   software_order_id,
   classification,
   operator_priority,
   first_batch_eligible,
   hold_reason,
   next_action,
   customer_safe_status,
   customer_safe_summary,
   reviewer_user_id,
   reviewed_at,
   created_at,
   updated_at)
SELECT 'order_triage_aibtc_smoke',
       id,
       'smoke_or_test',
       210,
       0,
       'Testing order; not promoted to customer overnight work.',
       'Hold until an operator promotes this smoke order into a real scope.',
       'held',
       'This request is held while OpenAgents confirms whether it is a real task or a test.',
       NULL,
       datetime('now'),
       datetime('now'),
       datetime('now')
  FROM software_orders
 WHERE id = 'software_order_backfill_10948188';

INSERT OR IGNORE INTO order_triage_records
  (id,
   software_order_id,
   classification,
   operator_priority,
   first_batch_eligible,
   hold_reason,
   next_action,
   customer_safe_status,
   customer_safe_summary,
   reviewer_user_id,
   reviewed_at,
   created_at,
   updated_at)
SELECT 'order_triage_omega_smoke',
       id,
       'smoke_or_test',
       220,
       0,
       'Testing order; not promoted to customer overnight work.',
       'Hold as an order-pipeline smoke test unless an operator promotes it.',
       'held',
       'This request is held while OpenAgents confirms whether it is a real task or a test.',
       NULL,
       datetime('now'),
       datetime('now'),
       datetime('now')
  FROM software_orders
 WHERE id = 'software_order_993d773d82d24490888c98112365c2e5';

INSERT OR IGNORE INTO order_triage_records
  (id,
   software_order_id,
   classification,
   operator_priority,
   first_batch_eligible,
   hold_reason,
   next_action,
   customer_safe_status,
   customer_safe_summary,
   reviewer_user_id,
   reviewed_at,
   created_at,
   updated_at)
SELECT 'order_triage_minnesota_legal_review',
       id,
       'legal_sensitive_policy_review',
       500,
       0,
       'Legal-sensitive request; automatic overnight execution is blocked.',
       'Hold for explicit human policy review and customer-safe scope limits.',
       'policy_review',
       'This request needs human review before OpenAgents can determine whether it can continue.',
       NULL,
       datetime('now'),
       datetime('now'),
       datetime('now')
  FROM software_orders
 WHERE id = 'software_order_backfill_86986020';
