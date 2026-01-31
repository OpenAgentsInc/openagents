/**
 * Job envelope for queue messages.
 */
export interface IndexerJob {
  type: 'FETCH_COMMENTS' | 'BACKFILL_COMMENTS';
  post_id: string;
  cursor?: string;
  page?: number;
  attempt?: number;
}

export interface Env {
  DB: D1Database;
  R2: R2Bucket;
  STATE: KVNamespace;
  JOBS: Queue<IndexerJob>;
  OA_API?: string;
  MOLTBOOK_API_BASE?: string;
  MOLTBOOK_API_KEY?: string;
  INDEXER_AUTH_HEADER?: string; // optional bearer for /v1/search and /v1/metrics
}

export interface MoltbookPost {
  id?: string;
  created_at?: string;
  submolt?: string;
  title?: string;
  content?: string;
  url?: string;
  author_name?: string;
  author_id?: string;
  score?: number;
  comment_count?: number;
  [k: string]: unknown;
}

export interface MoltbookComment {
  id?: string;
  post_id?: string;
  parent_id?: string;
  created_at?: string;
  author_name?: string;
  author_id?: string;
  content?: string;
  score?: number;
  [k: string]: unknown;
}

export interface SecretScanResult {
  hasSecrets: boolean;
  redactedContent?: string;
}
