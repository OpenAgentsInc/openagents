-- Store the owner-authenticated result envelope for completed inference batch
-- jobs. Public closeout receipts still expose only receipt metadata; raw
-- completions are returned only through the authenticated job results route.

ALTER TABLE inference_batch_jobs ADD COLUMN results_json TEXT;
