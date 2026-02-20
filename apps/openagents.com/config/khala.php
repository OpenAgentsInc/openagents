<?php

return [
    'token' => [
        /*
        |--------------------------------------------------------------------------
        | Khala Auth Token Bridge
        |--------------------------------------------------------------------------
        |
        | Laravel mints short-lived Khala auth JWTs from authenticated OA session
        | identity. Khala validates these claims via its custom JWT auth config.
        |
        */
        'enabled' => (bool) env('OA_KHALA_TOKEN_ENABLED', true),
        'issuer' => (string) env('OA_KHALA_TOKEN_ISSUER', (string) env('APP_URL', '')),
        'audience' => (string) env('OA_KHALA_TOKEN_AUDIENCE', 'openagents-khala'),
        'ttl_seconds' => (int) env('OA_KHALA_TOKEN_TTL_SECONDS', 300),
        'min_ttl_seconds' => (int) env('OA_KHALA_TOKEN_MIN_TTL_SECONDS', 60),
        'max_ttl_seconds' => (int) env('OA_KHALA_TOKEN_MAX_TTL_SECONDS', 900),
        'subject_prefix' => (string) env('OA_KHALA_TOKEN_SUBJECT_PREFIX', 'user'),
        'key_id' => (string) env('OA_KHALA_TOKEN_KEY_ID', 'khala-auth-v1'),
        'claims_version' => (string) env('OA_KHALA_TOKEN_CLAIMS_VERSION', 'oa_khala_claims_v1'),

        // HS256 symmetric signing key (server-only secret).
        'signing_key' => (string) env('OA_KHALA_TOKEN_SIGNING_KEY', ''),
    ],
];
