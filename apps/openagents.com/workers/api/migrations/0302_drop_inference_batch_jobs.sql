-- Wave 1 cleanup #8384: batch jobs stayed default-off and had no Khala Code
-- dependency, so the inert route/queue surface was removed.
DROP TABLE IF EXISTS inference_batch_jobs;
