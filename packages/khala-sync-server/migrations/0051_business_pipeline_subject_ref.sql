-- OB-2 Apollo sourcing mirror parity (#8559).
--
-- Mirrors the D1 business_pipeline_rows.subject_ref column as an opaque,
-- public-safe prospect subject identifier. D1 remains write-authority and
-- makes the uniqueness/dedupe decision; Postgres only converges accepted rows.

ALTER TABLE business_pipeline_rows
  ADD COLUMN IF NOT EXISTS subject_ref text;

CREATE UNIQUE INDEX IF NOT EXISTS business_pipeline_rows_subject_ref_idx
  ON business_pipeline_rows(subject_ref)
  WHERE subject_ref IS NOT NULL;
