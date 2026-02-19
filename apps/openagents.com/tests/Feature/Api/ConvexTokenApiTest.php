<?php

use App\Models\User;
use Illuminate\Support\Carbon;

it('requires sanctum authentication for convex token minting', function () {
    $this->postJson('/api/convex/token')->assertUnauthorized();
});

it('mints short-lived convex jwt from authenticated openagents session', function () {
    config()->set('convex.token.enabled', true);
    config()->set('convex.token.signing_key', 'convex-test-signing-key');
    config()->set('convex.token.issuer', 'https://openagents.test');
    config()->set('convex.token.audience', 'openagents-convex-test');
    config()->set('convex.token.ttl_seconds', 300);
    config()->set('convex.token.min_ttl_seconds', 60);
    config()->set('convex.token.max_ttl_seconds', 900);
    config()->set('convex.token.subject_prefix', 'user');
    config()->set('convex.token.key_id', 'convex-auth-test-v1');
    config()->set('convex.token.claims_version', 'oa_convex_claims_v1');

    $user = User::factory()->create([
        'email' => 'convex-token-user@openagents.com',
    ]);

    $apiToken = $user->createToken('convex-token-test')->plainTextToken;

    $response = $this->withToken($apiToken)
        ->postJson('/api/convex/token', [
            'scope' => ['codex:read', 'codex:write'],
            'workspace_id' => 'workspace_42',
            'role' => 'admin',
        ]);

    $response->assertOk()
        ->assertJsonPath('data.token_type', 'Bearer')
        ->assertJsonPath('data.issuer', 'https://openagents.test')
        ->assertJsonPath('data.audience', 'openagents-convex-test')
        ->assertJsonPath('data.subject', 'user:'.$user->id)
        ->assertJsonPath('data.claims_version', 'oa_convex_claims_v1')
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
    expect($decodedHeader['kid'] ?? null)->toBe('convex-auth-test-v1');

    expect($decodedPayload['iss'] ?? null)->toBe('https://openagents.test');
    expect($decodedPayload['aud'] ?? null)->toBe('openagents-convex-test');
    expect($decodedPayload['sub'] ?? null)->toBe('user:'.$user->id);
    expect($decodedPayload['oa_user_id'] ?? null)->toBe((int) $user->id);
    expect($decodedPayload['scope'] ?? [])->toBe(['codex:read', 'codex:write']);
    expect($decodedPayload['oa_claims_version'] ?? null)->toBe('oa_convex_claims_v1');
    expect($decodedPayload['oa_workspace_id'] ?? null)->toBe('workspace_42');
    expect($decodedPayload['oa_role'] ?? null)->toBe('admin');

    $issuedAt = (int) ($decodedPayload['iat'] ?? 0);
    $expiresAt = (int) ($decodedPayload['exp'] ?? 0);

    expect($issuedAt)->toBeGreaterThan(0);
    expect($expiresAt)->toBeGreaterThan($issuedAt);
    expect($expiresAt - $issuedAt)->toBe(300);
});

it('returns service unavailable when convex token bridge is misconfigured', function () {
    config()->set('convex.token.enabled', true);
    config()->set('convex.token.signing_key', '');
    config()->set('convex.token.issuer', 'https://openagents.test');
    config()->set('convex.token.audience', 'openagents-convex-test');

    $user = User::factory()->create([
        'email' => 'convex-token-misconfigured@openagents.com',
    ]);

    $apiToken = $user->createToken('convex-token-misconfigured')->plainTextToken;

    $this->withToken($apiToken)
        ->postJson('/api/convex/token')
        ->assertStatus(503)
        ->assertJsonPath('error.code', 'convex_token_unavailable');
});

it('reissues convex token with a fresh expiry for refresh behavior', function () {
    config()->set('convex.token.enabled', true);
    config()->set('convex.token.signing_key', 'convex-test-signing-key');
    config()->set('convex.token.issuer', 'https://openagents.test');
    config()->set('convex.token.audience', 'openagents-convex-test');
    config()->set('convex.token.ttl_seconds', 120);
    config()->set('convex.token.min_ttl_seconds', 60);
    config()->set('convex.token.max_ttl_seconds', 900);

    $user = User::factory()->create([
        'email' => 'convex-token-refresh@openagents.com',
    ]);

    $apiToken = $user->createToken('convex-token-refresh')->plainTextToken;

    try {
        Carbon::setTestNow(Carbon::parse('2026-02-19T12:00:00Z'));

        $firstResponse = $this->withToken($apiToken)->postJson('/api/convex/token')->assertOk();
        $firstPayload = decodeJwtSegment(explode('.', (string) $firstResponse->json('data.token'))[1]);

        Carbon::setTestNow(Carbon::parse('2026-02-19T12:05:00Z'));

        $secondResponse = $this->withToken($apiToken)->postJson('/api/convex/token')->assertOk();
        $secondPayload = decodeJwtSegment(explode('.', (string) $secondResponse->json('data.token'))[1]);

        expect((int) ($secondPayload['iat'] ?? 0))->toBeGreaterThan((int) ($firstPayload['iat'] ?? 0));
        expect((int) ($secondPayload['exp'] ?? 0))->toBeGreaterThan((int) ($firstPayload['exp'] ?? 0));
        expect((int) ($secondPayload['exp'] ?? 0) - (int) ($secondPayload['iat'] ?? 0))->toBe(120);
    } finally {
        Carbon::setTestNow();
    }
});

it('denies convex token minting when openagents api token is expired', function () {
    config()->set('convex.token.enabled', true);
    config()->set('convex.token.signing_key', 'convex-test-signing-key');
    config()->set('convex.token.issuer', 'https://openagents.test');
    config()->set('convex.token.audience', 'openagents-convex-test');

    $user = User::factory()->create([
        'email' => 'convex-token-expired@openagents.com',
    ]);

    $expiredApiToken = $user
        ->createToken('convex-token-expired', ['*'], now()->subMinute())
        ->plainTextToken;

    $this->withToken($expiredApiToken)
        ->postJson('/api/convex/token')
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
