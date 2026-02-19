<?php

use App\Models\User;
use App\Models\UserIntegration;
use Illuminate\Support\Str;

beforeEach(function () {
    config()->set('runtime.internal.shared_secret', 'test-runtime-internal-secret');
    config()->set('runtime.internal.key_id', 'runtime-internal-v1');
    config()->set('runtime.internal.signature_ttl_seconds', 60);
    config()->set('runtime.internal.secret_fetch_path', '/api/internal/runtime/integrations/secrets/fetch');
    config()->set('runtime.internal.secret_cache_ttl_ms', 45000);
});

test('runtime internal secret fetch returns scoped secret for valid signed request', function () {
    $user = User::factory()->create();

    UserIntegration::query()->create([
        'user_id' => $user->id,
        'provider' => 'resend',
        'status' => 'active',
        'encrypted_secret' => 're_live_1234567890',
        'secret_fingerprint' => hash('sha256', 're_live_1234567890'),
        'secret_last4' => '7890',
        'connected_at' => now(),
    ]);

    $payload = [
        'user_id' => $user->id,
        'provider' => 'resend',
        'integration_id' => 'resend.primary',
        'run_id' => 'run_123',
        'tool_call_id' => 'tool_123',
        'org_id' => 'org_abc',
    ];

    $signed = runtime_internal_sign($payload);

    $response = $this->call(
        'POST',
        (string) config('runtime.internal.secret_fetch_path'),
        [],
        [],
        [],
        runtime_internal_server_headers($signed['headers']),
        $signed['body'],
    );

    $response
        ->assertOk()
        ->assertJsonPath('data.provider', 'resend')
        ->assertJsonPath('data.secret', 're_live_1234567890')
        ->assertJsonPath('data.cache_ttl_ms', 45000)
        ->assertJsonPath('data.scope.user_id', $user->id)
        ->assertJsonPath('data.scope.integration_id', 'resend.primary')
        ->assertJsonPath('data.scope.run_id', 'run_123')
        ->assertJsonPath('data.scope.tool_call_id', 'tool_123')
        ->assertJsonPath('data.scope.org_id', 'org_abc');
});

test('runtime internal secret fetch supports google provider payloads', function () {
    $user = User::factory()->create();

    $googlePayload = json_encode([
        'provider' => 'google',
        'integration_id' => 'gmail.primary',
        'refresh_token' => '1//refresh_google_1234',
        'access_token' => 'ya29.google-access',
    ], JSON_THROW_ON_ERROR);

    UserIntegration::query()->create([
        'user_id' => $user->id,
        'provider' => 'google',
        'status' => 'active',
        'encrypted_secret' => $googlePayload,
        'secret_fingerprint' => hash('sha256', '1//refresh_google_1234'),
        'secret_last4' => '1234',
        'connected_at' => now(),
    ]);

    $payload = [
        'user_id' => $user->id,
        'provider' => 'google',
        'integration_id' => 'gmail.primary',
        'run_id' => 'run_google_123',
        'tool_call_id' => 'tool_google_123',
    ];

    $signed = runtime_internal_sign($payload);

    $response = $this->call(
        'POST',
        (string) config('runtime.internal.secret_fetch_path'),
        [],
        [],
        [],
        runtime_internal_server_headers($signed['headers']),
        $signed['body'],
    );

    $response
        ->assertOk()
        ->assertJsonPath('data.provider', 'google')
        ->assertJsonPath('data.secret', $googlePayload)
        ->assertJsonPath('data.scope.provider', 'google')
        ->assertJsonPath('data.scope.integration_id', 'gmail.primary');
});

test('runtime internal secret fetch rejects invalid signature', function () {
    $user = User::factory()->create();

    UserIntegration::query()->create([
        'user_id' => $user->id,
        'provider' => 'resend',
        'status' => 'active',
        'encrypted_secret' => 're_live_1234567890',
        'secret_fingerprint' => hash('sha256', 're_live_1234567890'),
        'secret_last4' => '7890',
        'connected_at' => now(),
    ]);

    $payload = [
        'user_id' => $user->id,
        'provider' => 'resend',
        'integration_id' => 'resend.primary',
        'run_id' => 'run_123',
        'tool_call_id' => 'tool_123',
    ];

    $signed = runtime_internal_sign($payload);
    $signed['headers']['x-oa-internal-signature'] = 'invalid-signature';

    $response = $this->call(
        'POST',
        (string) config('runtime.internal.secret_fetch_path'),
        [],
        [],
        [],
        runtime_internal_server_headers($signed['headers']),
        $signed['body'],
    );

    $response
        ->assertStatus(401)
        ->assertJsonPath('error.code', 'invalid_signature');
});

test('runtime internal secret fetch rejects nonce replay', function () {
    $user = User::factory()->create();

    UserIntegration::query()->create([
        'user_id' => $user->id,
        'provider' => 'resend',
        'status' => 'active',
        'encrypted_secret' => 're_live_1234567890',
        'secret_fingerprint' => hash('sha256', 're_live_1234567890'),
        'secret_last4' => '7890',
        'connected_at' => now(),
    ]);

    $payload = [
        'user_id' => $user->id,
        'provider' => 'resend',
        'integration_id' => 'resend.primary',
        'run_id' => 'run_123',
        'tool_call_id' => 'tool_123',
    ];

    $signed = runtime_internal_sign($payload, [
        'nonce' => 'nonce-replay',
    ]);

    $path = (string) config('runtime.internal.secret_fetch_path');

    $this->call(
        'POST',
        $path,
        [],
        [],
        [],
        runtime_internal_server_headers($signed['headers']),
        $signed['body'],
    )->assertOk();

    $response = $this->call(
        'POST',
        $path,
        [],
        [],
        [],
        runtime_internal_server_headers($signed['headers']),
        $signed['body'],
    );

    $response
        ->assertStatus(401)
        ->assertJsonPath('error.code', 'nonce_replay');
});

test('runtime internal secret fetch returns not found immediately after integration revocation', function () {
    $user = User::factory()->create();

    $integration = UserIntegration::query()->create([
        'user_id' => $user->id,
        'provider' => 'resend',
        'status' => 'active',
        'encrypted_secret' => 're_live_1234567890',
        'secret_fingerprint' => hash('sha256', 're_live_1234567890'),
        'secret_last4' => '7890',
        'connected_at' => now(),
    ]);

    $payload = [
        'user_id' => $user->id,
        'provider' => 'resend',
        'integration_id' => 'resend.primary',
        'run_id' => 'run_123',
        'tool_call_id' => 'tool_123',
    ];

    $path = (string) config('runtime.internal.secret_fetch_path');

    $initial = runtime_internal_sign($payload, ['nonce' => 'nonce-before-revoke']);

    $this->call(
        'POST',
        $path,
        [],
        [],
        [],
        runtime_internal_server_headers($initial['headers']),
        $initial['body'],
    )->assertOk();

    $integration->fill([
        'status' => 'inactive',
        'encrypted_secret' => null,
        'secret_fingerprint' => null,
        'secret_last4' => null,
        'disconnected_at' => now(),
    ]);
    $integration->save();

    $afterRevoke = runtime_internal_sign($payload, ['nonce' => 'nonce-after-revoke']);

    $response = $this->call(
        'POST',
        $path,
        [],
        [],
        [],
        runtime_internal_server_headers($afterRevoke['headers']),
        $afterRevoke['body'],
    );

    $response
        ->assertNotFound()
        ->assertJsonPath('error.code', 'secret_not_found');
});

/**
 * @param  array<string, mixed>  $payload
 * @param  array<string, mixed>  $overrides
 * @return array{body: string, headers: array<string, string>}
 */
function runtime_internal_sign(array $payload, array $overrides = []): array
{
    $body = json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
    if ($body === false) {
        $body = '{}';
    }

    $timestamp = (string) ($overrides['timestamp'] ?? now()->unix());
    $nonce = (string) ($overrides['nonce'] ?? Str::uuid());
    $keyId = (string) ($overrides['key_id'] ?? config('runtime.internal.key_id', 'runtime-internal-v1'));
    $secret = (string) config('runtime.internal.shared_secret', '');

    $bodyHash = hash('sha256', $body);
    $signature = hash_hmac('sha256', implode("\n", [$timestamp, $nonce, $bodyHash]), $secret);

    return [
        'body' => $body,
        'headers' => [
            'x-oa-internal-key-id' => $keyId,
            'x-oa-internal-timestamp' => $timestamp,
            'x-oa-internal-nonce' => $nonce,
            'x-oa-internal-body-sha256' => $bodyHash,
            'x-oa-internal-signature' => $signature,
        ],
    ];
}

/**
 * @param  array<string, string>  $headers
 * @return array<string, string>
 */
function runtime_internal_server_headers(array $headers): array
{
    $server = [
        'CONTENT_TYPE' => 'application/json',
    ];

    foreach ($headers as $name => $value) {
        $server['HTTP_'.strtoupper(str_replace('-', '_', $name))] = $value;
    }

    return $server;
}
