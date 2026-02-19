<?php

use App\Models\User;
use Illuminate\Http\Client\Request as HttpRequest;
use Illuminate\Support\Facades\Http;

beforeEach(function () {
    config()->set('runtime.elixir.base_url', 'http://runtime.internal');
    config()->set('runtime.elixir.tools_execute_path', '/internal/v1/tools/execute');
    config()->set('runtime.elixir.signing_key', 'runtime-signing-key');
    config()->set('runtime.elixir.signing_key_id', 'runtime-v1');
    config()->set('runtime.elixir.signature_ttl_seconds', 60);
    config()->set('runtime.elixir.max_retries', 0);
    config()->set('runtime.elixir.retry_backoff_ms', 1);
});

test('runtime tools api proxies coding request through reusable runtime client', function () {
    $user = User::factory()->create();

    Http::fake([
        'http://runtime.internal/internal/v1/tools/execute' => Http::response([
            'data' => [
                'state' => 'succeeded',
                'decision' => 'allowed',
                'reason_code' => 'policy_allowed.default',
            ],
        ], 200),
    ]);

    $response = $this->actingAs($user)->postJson('/api/runtime/tools/execute', [
        'tool_pack' => 'coding.v1',
        'mode' => 'replay',
        'run_id' => 'run_tools_1',
        'thread_id' => 'thread_tools_1',
        'manifest' => sampleManifest(),
        'request' => [
            'integration_id' => 'github.primary',
            'operation' => 'get_issue',
            'repository' => 'OpenAgentsInc/openagents',
            'issue_number' => 1747,
        ],
        'policy' => [
            'authorization_id' => 'auth_123',
            'authorization_mode' => 'delegated_budget',
        ],
    ]);

    $response
        ->assertOk()
        ->assertJsonPath('data.state', 'succeeded')
        ->assertJsonPath('data.reason_code', 'policy_allowed.default');

    Http::assertSent(function (HttpRequest $request) use ($user): bool {
        $signature = (string) ($request->header('X-OA-RUNTIME-SIGNATURE')[0] ?? '');
        $claims = decodeRuntimeTokenClaims($signature);

        return $request->url() === 'http://runtime.internal/internal/v1/tools/execute'
            && $request->hasHeader('X-OA-RUNTIME-SIGNATURE')
            && $request->hasHeader('X-OA-USER-ID')
            && ($request->header('X-OA-USER-ID')[0] ?? null) === (string) $user->id
            && ($claims['user_id'] ?? null) === $user->id
            && ($claims['run_id'] ?? null) === 'run_tools_1'
            && ($claims['thread_id'] ?? null) === 'thread_tools_1'
            && $request['user_id'] === $user->id
            && $request['request']['user_id'] === $user->id
            && $request['request']['run_id'] === 'run_tools_1'
            && $request['request']['thread_id'] === 'thread_tools_1';
    });
});

test('runtime tools api rejects user_id mismatch before forwarding', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)->postJson('/api/runtime/tools/execute', [
        'tool_pack' => 'coding.v1',
        'manifest' => sampleManifest(),
        'request' => [
            'integration_id' => 'github.primary',
            'operation' => 'get_issue',
            'repository' => 'OpenAgentsInc/openagents',
            'issue_number' => 1747,
        ],
        'user_id' => $user->id + 1,
    ]);

    $response
        ->assertStatus(403)
        ->assertJsonPath('error.code', 'forbidden');

    Http::assertNothingSent();
});

test('runtime tools api returns runtime validation failures', function () {
    $user = User::factory()->create();

    Http::fake([
        'http://runtime.internal/internal/v1/tools/execute' => Http::response([
            'error' => [
                'code' => 'invalid_request',
                'message' => 'tool invocation validation failed',
                'details' => ['manifest_version is required'],
            ],
        ], 422),
    ]);

    $response = $this->actingAs($user)->postJson('/api/runtime/tools/execute', [
        'tool_pack' => 'coding.v1',
        'manifest' => sampleManifest(),
        'request' => [
            'integration_id' => 'github.primary',
            'operation' => 'get_issue',
            'repository' => 'OpenAgentsInc/openagents',
            'issue_number' => 1747,
        ],
    ]);

    $response
        ->assertStatus(422)
        ->assertJsonPath('error.code', 'invalid_request')
        ->assertJsonPath('error.details.0', 'manifest_version is required');
});

test('runtime tools api accepts manifest_ref-only payloads', function () {
    $user = User::factory()->create();

    Http::fake([
        'http://runtime.internal/internal/v1/tools/execute' => Http::response([
            'data' => ['state' => 'succeeded'],
        ], 200),
    ]);

    $response = $this->actingAs($user)->postJson('/api/runtime/tools/execute', [
        'tool_pack' => 'coding.v1',
        'manifest_ref' => ['integration_id' => 'github.primary'],
        'request' => [
            'integration_id' => 'github.primary',
            'operation' => 'get_issue',
            'repository' => 'OpenAgentsInc/openagents',
            'issue_number' => 1747,
        ],
    ]);

    $response->assertOk()->assertJsonPath('data.state', 'succeeded');

    Http::assertSent(function (HttpRequest $request): bool {
        return $request->url() === 'http://runtime.internal/internal/v1/tools/execute'
            && $request['manifest_ref']['integration_id'] === 'github.primary'
            && $request['manifest'] === [];
    });
});

test('runtime tools api requires manifest or manifest_ref', function () {
    $user = User::factory()->create();

    $response = $this->actingAs($user)->postJson('/api/runtime/tools/execute', [
        'tool_pack' => 'coding.v1',
        'request' => [
            'integration_id' => 'github.primary',
            'operation' => 'get_issue',
            'repository' => 'OpenAgentsInc/openagents',
            'issue_number' => 1747,
        ],
    ]);

    $response
        ->assertStatus(422)
        ->assertJsonPath('error.code', 'invalid_request');

    Http::assertNothingSent();
});

/**
 * @return array<string, mixed>
 */
function sampleManifest(): array
{
    return [
        'manifest_version' => 'coding.integration.v1',
        'integration_id' => 'github.primary',
        'provider' => 'github',
        'status' => 'active',
        'tool_pack' => 'coding.v1',
        'capabilities' => ['get_issue', 'get_pull_request', 'add_issue_comment'],
        'secrets_ref' => ['provider' => 'laravel', 'key_id' => 'intsec_github_1'],
        'policy' => [
            'write_operations_mode' => 'enforce',
            'max_requests_per_minute' => 240,
            'default_repository' => 'OpenAgentsInc/openagents',
        ],
    ];
}

/**
 * @return array<string, mixed>
 */
function decodeRuntimeTokenClaims(string $token): array
{
    $parts = explode('.', $token, 3);
    if (count($parts) !== 3 || $parts[0] !== 'v1') {
        return [];
    }

    $payload = base64_decode(strtr($parts[1], '-_', '+/'), true);
    if (! is_string($payload)) {
        return [];
    }

    $claims = json_decode($payload, true);

    return is_array($claims) ? $claims : [];
}
