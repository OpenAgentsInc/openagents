-- OFR-013 (#9295): `metric.cost_to_identification.v1` is defined as measured
-- provider *and* incremental infrastructure cost through immutable T5. Provider
-- cost already rides `provider_usage`; without a retained infrastructure receipt
-- the second half is unknown and the projector must report the metric
-- unavailable. Admit the receipt kind so the ledger, not a scorecard caller, is
-- where that half comes from.
--
-- 0134 wrote the kind list as an unnamed column CHECK, so its generated name is
-- not guaranteed. Drop whichever CHECK on this table constrains the kind list
-- rather than guessing a name and silently leaving the old one in force.

DO $$
DECLARE
  existing_constraint TEXT;
BEGIN
  SELECT conname INTO existing_constraint
    FROM pg_constraint
   WHERE conrelid = 'forensic_metric_evidence'::regclass
     AND contype = 'c'
     AND pg_get_constraintdef(oid) LIKE '%reviewer_burden%'
   LIMIT 1;

  IF existing_constraint IS NOT NULL THEN
    EXECUTE format(
      'ALTER TABLE forensic_metric_evidence DROP CONSTRAINT %I',
      existing_constraint
    );
  END IF;
END
$$;

ALTER TABLE forensic_metric_evidence
  DROP CONSTRAINT IF EXISTS forensic_metric_evidence_record_kind_admitted;

ALTER TABLE forensic_metric_evidence
  ADD CONSTRAINT forensic_metric_evidence_record_kind_admitted
  CHECK (
    record_kind IN (
      'run_event',
      'provider_usage',
      'adjudication',
      'reviewer_burden',
      'infrastructure_cost'
    )
  );
