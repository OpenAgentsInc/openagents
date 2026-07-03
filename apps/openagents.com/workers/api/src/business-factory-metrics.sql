-- BF-7.2 locked business factory + engagement metric queries.
--
-- Bind parameters:
--   ?1 = inclusive window_start ISO timestamp
--   ?2 = exclusive window_end ISO timestamp
--
-- The query emits a normalized row set matching
-- docs/fable/2026-07-03-bf-7-2-locked-business-factory-metrics.md.
-- It intentionally returns not_measured rows for unauditable empty-rate
-- windows instead of fabricating zeros.

WITH
  params(window_start, window_end) AS (
    SELECT ?1, ?2
  ),
  work_kinds(work_kind) AS (
    VALUES
      ('adjustment'),
      ('business'),
      ('coding'),
      ('existing_project_import'),
      ('legal_sensitive'),
      ('site')
  ),
  accepted_contracts AS (
    SELECT
      c.work_kind,
      c.id,
      c.created_at,
      c.updated_at
    FROM omni_accepted_outcome_contracts c, params p
    WHERE c.archived_at IS NULL
      AND c.acceptance_state = 'accepted'
      AND c.updated_at >= p.window_start
      AND c.updated_at < p.window_end
  ),
  terminal_contracts AS (
    SELECT
      c.work_kind,
      c.acceptance_state,
      c.id
    FROM omni_accepted_outcome_contracts c, params p
    WHERE c.archived_at IS NULL
      AND c.acceptance_state IN (
        'accepted',
        'rejected',
        'revision_requested',
        'unavailable'
      )
      AND c.updated_at >= p.window_start
      AND c.updated_at < p.window_end
  ),
  economics_rows AS (
    SELECT
      e.id,
      e.work_kind,
      e.accepted_outcome_contract_id,
      e.review_minutes,
      e.updated_at
    FROM omni_accepted_outcome_economics e, params p
    WHERE e.archived_at IS NULL
      AND e.updated_at >= p.window_start
      AND e.updated_at < p.window_end
  ),
  engagement_review_rows AS (
    SELECT
      COALESCE(NULLIF(c.customer_ref, ''), c.subject_ref) AS engagement_ref,
      e.review_minutes,
      e.id AS economics_id
    FROM economics_rows e
    INNER JOIN omni_accepted_outcome_contracts c
      ON c.id = e.accepted_outcome_contract_id
    WHERE c.archived_at IS NULL
      AND COALESCE(NULLIF(c.customer_ref, ''), c.subject_ref) IS NOT NULL
  )
SELECT
  'business_factory.throughput.accepted_outcomes.v1' AS metric_ref,
  'accepted outcome throughput' AS metric_name,
  'work_kind' AS grain,
  wk.work_kind AS work_kind,
  NULL AS engagement_ref,
  p.window_start AS window_start,
  p.window_end AS window_end,
  COUNT(ac.id) AS numerator,
  NULL AS denominator,
  COUNT(ac.id) AS value,
  'outcomes' AS unit,
  'measured' AS measurement_state,
  '["table.omni_accepted_outcome_contracts"]' AS evidence_refs_json,
  '[]' AS caveat_refs_json
FROM work_kinds wk
CROSS JOIN params p
LEFT JOIN accepted_contracts ac
  ON ac.work_kind = wk.work_kind
GROUP BY wk.work_kind

UNION ALL

SELECT
  'business_factory.cycle_time.accepted_minutes.v1' AS metric_ref,
  'accepted outcome cycle time' AS metric_name,
  'work_kind' AS grain,
  wk.work_kind AS work_kind,
  NULL AS engagement_ref,
  p.window_start AS window_start,
  p.window_end AS window_end,
  COUNT(ac.id) AS numerator,
  COUNT(ac.id) AS denominator,
  CASE
    WHEN COUNT(ac.id) = 0 THEN NULL
    ELSE ROUND(AVG((julianday(ac.updated_at) - julianday(ac.created_at)) * 1440.0), 2)
  END AS value,
  'minutes' AS unit,
  CASE WHEN COUNT(ac.id) = 0 THEN 'not_measured' ELSE 'measured' END AS measurement_state,
  '["table.omni_accepted_outcome_contracts"]' AS evidence_refs_json,
  CASE
    WHEN COUNT(ac.id) = 0
      THEN '["caveat.business_metrics.no_accepted_outcomes_in_window"]'
    ELSE '[]'
  END AS caveat_refs_json
FROM work_kinds wk
CROSS JOIN params p
LEFT JOIN accepted_contracts ac
  ON ac.work_kind = wk.work_kind
GROUP BY wk.work_kind

UNION ALL

SELECT
  'business_factory.pass_rate.terminal_outcomes_bps.v1' AS metric_ref,
  'terminal outcome pass rate' AS metric_name,
  'work_kind' AS grain,
  wk.work_kind AS work_kind,
  NULL AS engagement_ref,
  p.window_start AS window_start,
  p.window_end AS window_end,
  SUM(CASE WHEN tc.acceptance_state = 'accepted' THEN 1 ELSE 0 END) AS numerator,
  COUNT(tc.id) AS denominator,
  CASE
    WHEN COUNT(tc.id) = 0 THEN NULL
    ELSE ROUND(
      SUM(CASE WHEN tc.acceptance_state = 'accepted' THEN 1 ELSE 0 END) * 10000.0
      / COUNT(tc.id),
      0
    )
  END AS value,
  'basis_points' AS unit,
  CASE WHEN COUNT(tc.id) = 0 THEN 'not_measured' ELSE 'measured' END AS measurement_state,
  '["table.omni_accepted_outcome_contracts"]' AS evidence_refs_json,
  CASE
    WHEN COUNT(tc.id) = 0
      THEN '["caveat.business_metrics.no_terminal_outcomes_in_window"]'
    ELSE '[]'
  END AS caveat_refs_json
FROM work_kinds wk
CROSS JOIN params p
LEFT JOIN terminal_contracts tc
  ON tc.work_kind = wk.work_kind
GROUP BY wk.work_kind

UNION ALL

SELECT
  'business_factory.review_minutes.v1' AS metric_ref,
  'ledgered review minutes' AS metric_name,
  'work_kind' AS grain,
  wk.work_kind AS work_kind,
  NULL AS engagement_ref,
  p.window_start AS window_start,
  p.window_end AS window_end,
  COALESCE(SUM(er.review_minutes), 0) AS numerator,
  COUNT(er.accepted_outcome_contract_id) AS denominator,
  COALESCE(SUM(er.review_minutes), 0) AS value,
  'minutes' AS unit,
  'measured' AS measurement_state,
  '["table.omni_accepted_outcome_economics"]' AS evidence_refs_json,
  '[]' AS caveat_refs_json
FROM work_kinds wk
CROSS JOIN params p
LEFT JOIN economics_rows er
  ON er.work_kind = wk.work_kind
GROUP BY wk.work_kind

UNION ALL

SELECT
  'business_engagement.operator_minutes.review_ledger_floor.v1' AS metric_ref,
  'operator minutes per engagement review-ledger floor' AS metric_name,
  'engagement' AS grain,
  NULL AS work_kind,
  engagement_ref AS engagement_ref,
  p.window_start AS window_start,
  p.window_end AS window_end,
  SUM(review_minutes) AS numerator,
  COUNT(economics_id) AS denominator,
  SUM(review_minutes) AS value,
  'minutes' AS unit,
  'measured' AS measurement_state,
  '["table.omni_accepted_outcome_economics","table.omni_accepted_outcome_contracts"]' AS evidence_refs_json,
  '["caveat.business_metrics.operator_minutes_review_only_until_labor_ledger"]' AS caveat_refs_json
FROM engagement_review_rows, params p
GROUP BY engagement_ref
ORDER BY metric_ref, grain, work_kind, engagement_ref;
