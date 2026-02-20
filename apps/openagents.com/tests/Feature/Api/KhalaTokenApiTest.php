<?php

use App\Models\User;
use Illuminate\Support\Carbon;

it('requires sanctum authentication for khala token minting', function () {
    $this->postJson('/api/khala/token')->assertUnauthorized();
});

it('mints short-lived khala jwt from authenticated openagents session', function () {
    config()->set('khala.token.enabled', true);
    config()->set('khala.token.signing_key', 'khala-test-signing-key');
    config()->set('khala.token.issuer', 'https://openagents.test');
    config()->set('khala.token.audience', 'openagents-khala-test');
    config()->set('khala.token.ttl_seconds', 300);
    config()->set('khala.token.min_ttl_seconds', 60);
    config()->set('khala.token.max_ttl_seconds', 900);
    config()->set('khala.token.subject_prefix', 'user');
    config()->set('khala.token.key_id', 'khala-auth-test-v1');
    config()->set('khala.token.claims_version', 'oa_khala_claims_v1');

    $user = User::factory()->create([
        'email' => 'khala-token-user@openagents.com',
    ]);

    $apiToken = $user->createToken('khala-token-test')->plainTextToken;

    $response = $this->withToken($apiToken)
        ->postJson('/api/khala/token', [
            'scope' => ['codex:read', 'codex:write'],
            'workspace_id' => 'workspace_42',
            'role' => 'admin',
        ]);

    $response->assertOk()
        ->assertJsonPath('data.token_type', 'Bearer')
        ->assertJsonPath('data.issuer', 'https://openagents.test')
        ->assertJsonPath('data.audience', 'openagents-khala-test')
        ->assertJsonPath('data.subject', 'user:'.$user->id)
        ->assertJsonPath('data.claims_version', 'oa_khala_claims_v1')
        ->assertJsonPath('data.scope.0', 'codex:read')
        ->assertJsonPath('data.scope.1', 'codex:write')
        ->assertJsonPath('data.workspace_id', 'workspace_42')
        ->assertJsonPath('data.role', 'admin');

    $jwt = (string) $response->json('data.token');
    expect($jwt)->not->toBe('');

    [$header, $payload, $signature] = explode('.', $jwt);

    $decodedHeader = decodeJwtSegment($header);
    $decodedPayload = decodeJwtSegment($payload);

    expect($signature)->not->toBe('');
    expect($decodedHeader['alg'] ?? null)->toBe('HS256');
    expect($decodedHeader['kid'] ?? null)->toBe('khala-auth-test-v1');

    expect($decodedPayload['iss'] ?? null)->toBe('https://openagents.test');
    expect($decodedPayload['aud'] ?? null)->toBe('openagents-khala-test');
    expect($decodedPayload['sub'] ?? null)->toBe('user:'.$user->id);
    expect($decodedPayload['oa_user_id'] ?? null)->toBe((int) $user->id);
    expect($decodedPayload['scope'] ?? [])->toBe(['codex:read', 'codex:write']);
    expect($decodedPayload['oa_claims_version'] ?? null)->toBe('oa_khala_claims_v1');
    expect($decodedPayload['oa_workspace_id'] ?? null)->toBe('workspace_42');
    expect($decodedPayload['oa_role'] ?? null)->toBe('admin');

    $issuedAt = (int) ($decodedPayload['iat'] ?? 0);
    $expiresAt = (int) ($decodedPayload['exp'] ?? 0);

    expect($issuedAt)->toBeGreaterThan(0);
    expect($expiresAt)->toBeGreaterThan($issuedAt);
    expect($expiresAt - $issuedAt)->toBe(300);
});

it('returns service unavailable when khala token bridge is misconfigured', function () {
    config()->set('khala.token.enabled', true);
    config()->set('khala.token.signing_key', '');
    config()->set('khala.token.issuer', 'https://openagents.test');
    config()->set('khala.token.audience', 'openagents-khala-test');

    $user = User::factory()->create([
        'email' => 'khala-token-misconfigured@openagents.com',
    ]);

    $apiToken = $user->createToken('khala-token-misconfigured')->plainTextToken;

    $this->withToken($apiToken)
        ->postJson('/api/khala/token')
        ->assertStatus(503)
        ->assertJsonPath('error.code', 'khala_token_unavailable');
});

it('reissues khala token with a fresh expiry for refresh behavior', function () {
    config()->set('khala.token.enabled', true);
    config()->set('khala.token.signing_key', 'khala-test-signing-key');
    config()->set('khala.token.issuer', 'https://openagents.test');
    config()->set('khala.token.audience', 'openagents-khala-test');
    config()->set('khala.token.ttl_seconds', 120);
    config()->set('khala.token.min_ttl_seconds', 60);
    config()->set('khala.token.max_ttl_seconds', 900);

    $user = User::factory()->create([
        'email' => 'khala-token-refresh@openagents.com',
    ]);

    $apiToken = $user->createToken('khala-token-refresh')->plainTextToken;

    try {
        Carbon::setTestNow(Carbon::parse('2026-02-19T12:00:00Z'));

        $firstResponse = $this->withToken($apiToken)->postJson('/api/khala/token')->assertOk();
        $firstPayload = decodeJwtSegment(explode('.', (string) $firstResponse->json('data.token'))[1]);

        Carbon::setTestNow(Carbon::parse('2026-02-19T12:05:00Z'));

        $secondResponse = $this->withToken($apiToken)->postJson('/api/khala/token')->assertOk();
        $secondPayload = decodeJwtSegment(explode('.', (string) $secondResponse->json('data.token'))[1]);

        expect((int) ($secondPayload['iat'] ?? 0))->toBeGreaterThan((int) ($firstPayload['iat'] ?? 0));
        expect((int) ($secondPayload['exp'] ?? 0))->toBeGreaterThan((int) ($firstPayload['exp'] ?? 0));
        expect((int) ($secondPayload['exp'] ?? 0) - (int) ($secondPayload['iat'] ?? 0))->toBe(120);
    } finally {
        Carbon::setTestNow();
    }
});

it('denies khala token minting when openagents api token is expired', function () {
    config()->set('khala.token.enabled', true);
    config()->set('khala.token.signing_key', 'khala-test-signing-key');
    config()->set('khala.token.issuer', 'https://openagents.test');
    config()->set('khala.token.audience', 'openagents-khala-test');

    $user = User::factory()->create([
        'email' => 'khala-token-expired@openagents.com',
    ]);

    $expiredApiToken = $user
        ->createToken('khala-token-expired', ['*'], now()->subMinute())
        ->plainTextToken;

    $this->withToken($expiredApiToken)
        ->postJson('/api/khala/token')
        ->assertUnauthorized();
});

/**
 * @return array<string, mixed>
 */
function decodeJwtSegment(string $segment): array
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
