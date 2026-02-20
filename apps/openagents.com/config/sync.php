<?php

$parseCsv = static function (string $value): array {
    return array_values(array_filter(array_map(
        static fn (string $item): string => trim($item),
        explode(',', $value)
    ), static fn (string $item): bool => $item !== ''));
};

$defaultAllowedScopes = 'runtime.run_summaries,runtime.codex_worker_summaries,runtime.notifications';

return [
    'token' => [
        /*
        |--------------------------------------------------------------------------
        | Khala Sync Token Bridge
        |--------------------------------------------------------------------------
        |
        | Laravel mints short-lived Khala auth JWTs from authenticated OA session
        | identity. Runtime will validate these JWTs for sync socket access.
        |
        */
        'enabled' => (bool) env('OA_SYNC_TOKEN_ENABLED', true),
        'issuer' => (string) env('OA_SYNC_TOKEN_ISSUER', (string) env('APP_URL', '')),
        'audience' => (string) env('OA_SYNC_TOKEN_AUDIENCE', 'openagents-sync'),
        'ttl_seconds' => (int) env('OA_SYNC_TOKEN_TTL_SECONDS', 300),
        'min_ttl_seconds' => (int) env('OA_SYNC_TOKEN_MIN_TTL_SECONDS', 60),
        'max_ttl_seconds' => (int) env('OA_SYNC_TOKEN_MAX_TTL_SECONDS', 900),
        'subject_prefix' => (string) env('OA_SYNC_TOKEN_SUBJECT_PREFIX', 'user'),
        'org_prefix' => (string) env('OA_SYNC_TOKEN_ORG_PREFIX', 'user'),
        'key_id' => (string) env('OA_SYNC_TOKEN_KEY_ID', 'sync-auth-v1'),
        'claims_version' => (string) env('OA_SYNC_TOKEN_CLAIMS_VERSION', 'oa_sync_claims_v1'),
        'default_scopes' => $parseCsv((string) env('OA_SYNC_TOKEN_DEFAULT_SCOPES', 'runtime.codex_worker_summaries')),
        'allowed_scopes' => $parseCsv((string) env('OA_SYNC_TOKEN_ALLOWED_SCOPES', $defaultAllowedScopes)),

        // HS256 symmetric signing key (server-only secret).
        'signing_key' => (string) env('OA_SYNC_TOKEN_SIGNING_KEY', ''),
    ],
];
