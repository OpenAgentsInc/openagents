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

WITH RECURSIVE
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
  revenue_products(product_ref) AS (
    VALUES
      ('khala_code'),
      ('qa_swarm')
  ),
  revenue_events AS (
    SELECT
      r.event_ref,
      r.product_ref,
      r.demand_provenance,
      r.payment_state,
      r.amount_cents,
      r.amount_sats,
      r.recorded_at
    FROM revenue_event_provenance r, params p
    WHERE r.recorded_at >= p.window_start
      AND r.recorded_at < p.window_end
      AND r.payment_state IN (
        'payment_evidence_recorded',
        'fulfilled',
        'settled'
      )
  ),
  engagement_review_rows AS (
    SELECT
      COALESCE(NULLIF(c.customer_ref, ''), c.subject_ref) AS engagement_ref,
      e.review_minutes,
      e.updated_at,
      e.id AS economics_id
    FROM economics_rows e
    INNER JOIN omni_accepted_outcome_contracts c
      ON c.id = e.accepted_outcome_contract_id
    WHERE c.archived_at IS NULL
      AND c.acceptance_state = 'accepted'
      AND COALESCE(NULLIF(c.customer_ref, ''), c.subject_ref) IS NOT NULL
  ),
  month_windows(month_start, month_end) AS (
    SELECT
      strftime('%Y-%m-01T00:00:00.000Z', p.window_start) AS month_start,
      strftime(
        '%Y-%m-01T00:00:00.000Z',
        datetime(p.window_start, 'start of month', '+1 month')
      ) AS month_end
    FROM params p
    UNION ALL
    SELECT
      mw.month_end AS month_start,
      strftime(
        '%Y-%m-01T00:00:00.000Z',
        datetime(mw.month_end, '+1 month')
      ) AS month_end
    FROM month_windows mw, params p
    WHERE mw.month_end < p.window_end
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
  'business_factory.revenue_events.external_count.v1' AS metric_ref,
  'external revenue event count' AS metric_name,
  'work_kind' AS grain,
  rp.product_ref AS work_kind,
  NULL AS engagement_ref,
  p.window_start AS window_start,
  p.window_end AS window_end,
  COUNT(re.event_ref) AS numerator,
  NULL AS denominator,
  COUNT(re.event_ref) AS value,
  'outcomes' AS unit,
  'measured' AS measurement_state,
  '["table.revenue_event_provenance"]' AS evidence_refs_json,
  '[]' AS caveat_refs_json
FROM revenue_products rp
CROSS JOIN params p
LEFT JOIN revenue_events re
  ON re.product_ref = rp.product_ref
  AND re.demand_provenance = 'external'
GROUP BY rp.product_ref

UNION ALL

SELECT
  'business_factory.revenue_events.internal_count.v1' AS metric_ref,
  'internal revenue event count' AS metric_name,
  'work_kind' AS grain,
  rp.product_ref AS work_kind,
  NULL AS engagement_ref,
  p.window_start AS window_start,
  p.window_end AS window_end,
  COUNT(re.event_ref) AS numerator,
  NULL AS denominator,
  COUNT(re.event_ref) AS value,
  'outcomes' AS unit,
  'measured' AS measurement_state,
  '["table.revenue_event_provenance"]' AS evidence_refs_json,
  '[]' AS caveat_refs_json
FROM revenue_products rp
CROSS JOIN params p
LEFT JOIN revenue_events re
  ON re.product_ref = rp.product_ref
  AND re.demand_provenance = 'internal'
GROUP BY rp.product_ref

UNION ALL

SELECT
  'business_factory.revenue_usd_cents.external.v1' AS metric_ref,
  'external revenue USD cents' AS metric_name,
  'work_kind' AS grain,
  rp.product_ref AS work_kind,
  NULL AS engagement_ref,
  p.window_start AS window_start,
  p.window_end AS window_end,
  COALESCE(SUM(CASE WHEN re.amount_cents IS NULL THEN 0 ELSE re.amount_cents END), 0) AS numerator,
  COUNT(re.event_ref) AS denominator,
  COALESCE(SUM(CASE WHEN re.amount_cents IS NULL THEN 0 ELSE re.amount_cents END), 0) AS value,
  'usd_cents' AS unit,
  'measured' AS measurement_state,
  '["table.revenue_event_provenance"]' AS evidence_refs_json,
  CASE
    WHEN COALESCE(SUM(CASE WHEN re.amount_cents IS NULL AND re.amount_sats IS NOT NULL THEN 1 ELSE 0 END), 0) > 0
      THEN '["caveat.business_metrics.sat_revenue_excluded_from_usd_cent_metric"]'
    ELSE '[]'
  END AS caveat_refs_json
FROM revenue_products rp
CROSS JOIN params p
LEFT JOIN revenue_events re
  ON re.product_ref = rp.product_ref
  AND re.demand_provenance = 'external'
GROUP BY rp.product_ref

UNION ALL

SELECT
  'business_factory.revenue_usd_cents.internal.v1' AS metric_ref,
  'internal revenue USD cents' AS metric_name,
  'work_kind' AS grain,
  rp.product_ref AS work_kind,
  NULL AS engagement_ref,
  p.window_start AS window_start,
  p.window_end AS window_end,
  COALESCE(SUM(CASE WHEN re.amount_cents IS NULL THEN 0 ELSE re.amount_cents END), 0) AS numerator,
  COUNT(re.event_ref) AS denominator,
  COALESCE(SUM(CASE WHEN re.amount_cents IS NULL THEN 0 ELSE re.amount_cents END), 0) AS value,
  'usd_cents' AS unit,
  'measured' AS measurement_state,
  '["table.revenue_event_provenance"]' AS evidence_refs_json,
  CASE
    WHEN COALESCE(SUM(CASE WHEN re.amount_cents IS NULL AND re.amount_sats IS NOT NULL THEN 1 ELSE 0 END), 0) > 0
      THEN '["caveat.business_metrics.sat_revenue_excluded_from_usd_cent_metric"]'
    ELSE '[]'
  END AS caveat_refs_json
FROM revenue_products rp
CROSS JOIN params p
LEFT JOIN revenue_events re
  ON re.product_ref = rp.product_ref
  AND re.demand_provenance = 'internal'
GROUP BY rp.product_ref

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

UNION ALL

SELECT
  'business_engagement.operator_minutes_per_engagement.monthly_review_ledger_floor.v1' AS metric_ref,
  'monthly operator minutes per accepted engagement review-ledger floor' AS metric_name,
  'window' AS grain,
  NULL AS work_kind,
  NULL AS engagement_ref,
  mw.month_start AS window_start,
  mw.month_end AS window_end,
  COALESCE(SUM(err.review_minutes), 0) AS numerator,
  COUNT(DISTINCT err.engagement_ref) AS denominator,
  CASE
    WHEN COUNT(DISTINCT err.engagement_ref) = 0 THEN NULL
    ELSE ROUND(
      COALESCE(SUM(err.review_minutes), 0) * 1.0
      / COUNT(DISTINCT err.engagement_ref),
      2
    )
  END AS value,
  'minutes' AS unit,
  CASE
    WHEN COUNT(DISTINCT err.engagement_ref) = 0 THEN 'not_measured'
    ELSE 'measured'
  END AS measurement_state,
  '["table.omni_accepted_outcome_economics","table.omni_accepted_outcome_contracts"]' AS evidence_refs_json,
  CASE
    WHEN COUNT(DISTINCT err.engagement_ref) = 0
      THEN '["caveat.business_metrics.no_accepted_engagements_in_month","caveat.business_metrics.operator_minutes_review_only_until_labor_ledger"]'
    ELSE '["caveat.business_metrics.operator_minutes_review_only_until_labor_ledger"]'
  END AS caveat_refs_json
FROM month_windows mw
LEFT JOIN engagement_review_rows err
  ON err.updated_at >= mw.month_start
  AND err.updated_at < mw.month_end
GROUP BY mw.month_start, mw.month_end
ORDER BY metric_ref, grain, work_kind, engagement_ref, window_start;
