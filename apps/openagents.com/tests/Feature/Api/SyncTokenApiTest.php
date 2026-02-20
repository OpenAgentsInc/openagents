<?php

use App\Models\User;

it('requires sanctum authentication for sync token minting', function () {
    $this->postJson('/api/sync/token')->assertUnauthorized();
});

it('mints short-lived sync jwt from authenticated openagents session', function () {
    config()->set('sync.token.enabled', true);
    config()->set('sync.token.signing_key', 'sync-test-signing-key');
    config()->set('sync.token.issuer', 'https://openagents.test');
    config()->set('sync.token.audience', 'openagents-sync-test');
    config()->set('sync.token.ttl_seconds', 300);
    config()->set('sync.token.min_ttl_seconds', 60);
    config()->set('sync.token.max_ttl_seconds', 900);
    config()->set('sync.token.subject_prefix', 'user');
    config()->set('sync.token.org_prefix', 'user');
    config()->set('sync.token.key_id', 'sync-auth-test-v1');
    config()->set('sync.token.claims_version', 'oa_sync_claims_v1');
    config()->set('sync.token.allowed_scopes', ['runtime.run_summaries', 'runtime.codex_worker_summaries']);
    config()->set('sync.token.default_scopes', ['runtime.codex_worker_summaries']);

    $user = User::factory()->create([
        'email' => 'sync-token-user@openagents.com',
    ]);

    $apiToken = $user->createToken('sync-token-test')->plainTextToken;

    $response = $this->withToken($apiToken)
        ->postJson('/api/sync/token', [
            'scopes' => ['runtime.codex_worker_summaries', 'runtime.run_summaries'],
        ]);

    $response->assertOk()
        ->assertJsonPath('data.token_type', 'Bearer')
        ->assertJsonPath('data.issuer', 'https://openagents.test')
        ->assertJsonPath('data.audience', 'openagents-sync-test')
        ->assertJsonPath('data.subject', 'user:'.$user->id)
        ->assertJsonPath('data.org_id', 'user:'.$user->id)
        ->assertJsonPath('data.claims_version', 'oa_sync_claims_v1')
        ->assertJsonPath('data.scopes.0', 'runtime.codex_worker_summaries')
        ->assertJsonPath('data.scopes.1', 'runtime.run_summaries')
        ->assertJsonPath('data.kid', 'sync-auth-test-v1');

    $jwt = (string) $response->json('data.token');
    expect($jwt)->not->toBe('');

    [$header, $payload, $signature] = explode('.', $jwt);

    $decodedHeader = decodeSyncJwtSegment($header);
    $decodedPayload = decodeSyncJwtSegment($payload);

    expect($signature)->not->toBe('');
    expect($decodedHeader['alg'] ?? null)->toBe('HS256');
    expect($decodedHeader['kid'] ?? null)->toBe('sync-auth-test-v1');

    expect($decodedPayload['iss'] ?? null)->toBe('https://openagents.test');
    expect($decodedPayload['aud'] ?? null)->toBe('openagents-sync-test');
    expect($decodedPayload['sub'] ?? null)->toBe('user:'.$user->id);
    expect($decodedPayload['oa_user_id'] ?? null)->toBe((int) $user->id);
    expect($decodedPayload['oa_org_id'] ?? null)->toBe('user:'.$user->id);
    expect($decodedPayload['oa_sync_scopes'] ?? [])->toBe(['runtime.codex_worker_summaries', 'runtime.run_summaries']);
    expect($decodedPayload['oa_claims_version'] ?? null)->toBe('oa_sync_claims_v1');

    $issuedAt = (int) ($decodedPayload['iat'] ?? 0);
    $expiresAt = (int) ($decodedPayload['exp'] ?? 0);

    expect($issuedAt)->toBeGreaterThan(0);
    expect($expiresAt)->toBeGreaterThan($issuedAt);
    expect($expiresAt - $issuedAt)->toBe(300);
});

it('returns service unavailable when sync token bridge is misconfigured', function () {
    config()->set('sync.token.enabled', true);
    config()->set('sync.token.signing_key', '');
    config()->set('sync.token.issuer', 'https://openagents.test');
    config()->set('sync.token.audience', 'openagents-sync-test');

    $user = User::factory()->create([
        'email' => 'sync-token-misconfigured@openagents.com',
    ]);

    $apiToken = $user->createToken('sync-token-misconfigured')->plainTextToken;

    $this->withToken($apiToken)
        ->postJson('/api/sync/token')
        ->assertStatus(503)
        ->assertJsonPath('error.code', 'sync_token_unavailable');
});

it('returns validation error for unknown sync scopes', function () {
    config()->set('sync.token.enabled', true);
    config()->set('sync.token.signing_key', 'sync-test-signing-key');
    config()->set('sync.token.issuer', 'https://openagents.test');
    config()->set('sync.token.audience', 'openagents-sync-test');
    config()->set('sync.token.allowed_scopes', ['runtime.run_summaries']);
    config()->set('sync.token.default_scopes', ['runtime.run_summaries']);

    $user = User::factory()->create([
        'email' => 'sync-token-invalid-scope@openagents.com',
    ]);

    $apiToken = $user->createToken('sync-token-invalid-scope')->plainTextToken;

    $this->withToken($apiToken)
        ->postJson('/api/sync/token', [
            'scopes' => ['runtime.unknown_topic'],
        ])
        ->assertStatus(422)
        ->assertJsonPath('error.code', 'invalid_scope');
});

it('denies sync token minting when openagents api token is expired', function () {
    config()->set('sync.token.enabled', true);
    config()->set('sync.token.signing_key', 'sync-test-signing-key');
    config()->set('sync.token.issuer', 'https://openagents.test');
    config()->set('sync.token.audience', 'openagents-sync-test');

    $user = User::factory()->create([
        'email' => 'sync-token-expired@openagents.com',
    ]);

    $expiredApiToken = $user
        ->createToken('sync-token-expired', ['*'], now()->subMinute())
        ->plainTextToken;

    $this->withToken($expiredApiToken)
        ->postJson('/api/sync/token')
        ->assertUnauthorized();
});

/**
 * @return array<string, mixed>
 */
function decodeSyncJwtSegment(string $segment): array
{
    $paddingLength = (4 - strlen($segment) % 4) % 4;
    $segment .= str_repeat('=', $paddingLength);
    $decoded = base64_decode(strtr($segment, '-_', '+/'));

    if (! is_string($decoded)) {
        return [];
    }

    $parsed = json_decode($decoded, true);

    return is_array($parsed) ? $parsed : [];
}
