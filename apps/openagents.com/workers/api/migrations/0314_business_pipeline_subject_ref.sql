-- OB-2 Apollo sourcing subject identity gate (#8559).
--
-- Stores only the public-safe opaque prospect subject ref that Apollo ingest
-- already uses for suppressions. No names, domains, emails, raw Apollo payloads,
-- or private contact data belong in this column.

ALTER TABLE business_pipeline_rows
  ADD COLUMN subject_ref TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_business_pipeline_rows_subject_ref
  ON business_pipeline_rows(subject_ref)
  WHERE subject_ref IS NOT NULL;
