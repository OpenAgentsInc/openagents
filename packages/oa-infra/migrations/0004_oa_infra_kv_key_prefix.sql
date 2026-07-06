-- oa-infra KvStore prefix scans (CFG-3, issue #8518).
--
-- `KvStoreShape.listPrefix` (the OpenAuth `StorageAdapter.scan` seam) runs
-- `key LIKE '<escaped-prefix>%'`. The PK btree index only serves LIKE
-- prefix scans under the C collation; databases created with a non-C
-- collation (Cloud SQL defaults) need an explicit text_pattern_ops index.

CREATE INDEX IF NOT EXISTS oa_infra_kv_key_prefix_idx
  ON oa_infra_kv (key text_pattern_ops);
