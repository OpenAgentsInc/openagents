CREATE TABLE IF NOT EXISTS inference_batch_jobs (
    job_id TEXT PRIMARY KEY,
    account_ref TEXT NOT NULL,
    status TEXT NOT NULL,
    charge_receipt_ref TEXT NOT NULL,
    dataset_size INTEGER NOT NULL,
    processed_items INTEGER NOT NULL DEFAULT 0,
    failed_items INTEGER NOT NULL DEFAULT 0,
    results_r2_key TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_inference_batch_jobs_account ON inference_batch_jobs (account_ref, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_inference_batch_jobs_status ON inference_batch_jobs (status, created_at ASC);
