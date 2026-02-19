<?php

use App\Models\User;

it('requires sanctum authentication for convex token minting', function () {
    $this->postJson('/api/convex/token')->assertUnauthorized();
});

it('mints short-lived convex jwt from authenticated openagents session', function () {
    config()->set('convex.token.enabled', true);
    config()->set('convex.token.signing_key', 'convex-test-signing-key');
    config()->set('convex.token.issuer', 'https://openagents.test');
    config()->set('convex.token.audience', 'openagents-convex-test');
    config()->set('convex.token.ttl_seconds', 300);
    config()->set('convex.token.subject_prefix', 'user');
    config()->set('convex.token.key_id', 'convex-auth-test-v1');

    $user = User::factory()->create([
        'email' => 'convex-token-user@openagents.com',
    ]);

    $apiToken = $user->createToken('convex-token-test')->plainTextToken;

    $response = $this->withToken($apiToken)
        ->postJson('/api/convex/token', [
            'scope' => ['codex:read', 'codex:write'],
        ]);

    $response->assertOk()
        ->assertJsonPath('data.token_type', 'Bearer')
        ->assertJsonPath('data.issuer', 'https://openagents.test')
        ->assertJsonPath('data.audience', 'openagents-convex-test')
        ->assertJsonPath('data.subject', 'user:'.$user->id)
        ->assertJsonPath('data.scope.0', 'codex:read')
        ->assertJsonPath('data.scope.1', 'codex:write');

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
