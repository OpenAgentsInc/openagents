<?php

use App\Jobs\ForwardResendWebhookToRuntime;
use App\Models\CommsDeliveryProjection;
use App\Models\User;
use App\Models\UserIntegration;
use App\Support\Comms\RuntimeCommsDeliveryForwarder;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Queue;
use Inertia\Testing\AssertableInertia as Assert;

beforeEach(function () {
    config()->set('runtime.comms.resend.webhook_secret', 'whsec_'.base64_encode('resend-webhook-test-secret'));
    config()->set('runtime.comms.resend.webhook_tolerance_seconds', 300);
    config()->set('runtime.comms.runtime_delivery_ingest_path', '/internal/v1/comms/delivery-events');
    config()->set('runtime.comms.runtime_delivery_max_retries', 0);
    config()->set('runtime.comms.runtime_delivery_retry_backoff_ms', 1);
    config()->set('runtime.comms.runtime_delivery_timeout_ms', 1000);
    config()->set('runtime.elixir.base_url', 'http://runtime.internal');
    config()->set('runtime.elixir.signing_key', 'runtime-signing-key');
    config()->set('runtime.elixir.signature_ttl_seconds', 60);
});

test('authored integration intent flows through runtime execution into single-writer projection for settings UI', function () {
    Queue::fake();

    $user = User::factory()->create();

    $this
        ->actingAs($user)
        ->post(route('settings.integrations.resend.upsert'), [
            'resend_api_key' => 're_test_intent_1234567890',
            'sender_email' => 'noreply@example.com',
            'sender_name' => 'OpenAgents',
        ])
        ->assertSessionHasNoErrors();

    $integration = UserIntegration::query()->where('user_id', $user->id)->where('provider', 'resend')->first();
    expect($integration)->not->toBeNull();
    expect((string) $integration->status)->toBe('active');

    $payloadArray = baseOwnershipPayload('email.delivered');
    $payloadArray['data']['tags'][] = ['name' => 'integration_id', 'value' => 'resend.primary'];
    $payloadArray['data']['tags'][] = ['name' => 'user_id', 'value' => (string) $user->id];
    $payloadArray['data']['email_id'] = 'email_ownership_flow';

    $payload = json_encode($payloadArray, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
    expect($payload)->toBeString();

    $headers = signedOwnershipWebhookHeaders((string) $payload, ['id' => 'evt_ownership_flow']);

    callOwnershipWebhook($this, (string) $payload, $headers)->assertStatus(202);

    $job = null;

    Queue::assertPushed(ForwardResendWebhookToRuntime::class, function (ForwardResendWebhookToRuntime $queued) use (&$job): bool {
        $job = $queued;

        return true;
    });

    expect($job)->toBeInstanceOf(ForwardResendWebhookToRuntime::class);

    Http::fake([
        'http://runtime.internal/internal/v1/comms/delivery-events' => Http::response([
            'eventId' => 'evt_ownership_flow',
            'status' => 'accepted',
            'idempotentReplay' => false,
        ], 202),
    ]);

    $job->handle(app(RuntimeCommsDeliveryForwarder::class), app(\App\Services\CommsDeliveryProjectionProjector::class));

    $projection = CommsDeliveryProjection::query()
        ->where('user_id', $user->id)
        ->where('provider', 'resend')
        ->where('integration_id', 'resend.primary')
        ->first();

    expect($projection)->not->toBeNull();
    expect((string) $projection->last_state)->toBe('delivered');
    expect((string) $projection->last_message_id)->toBe('email_ownership_flow');
    expect((string) $projection->source)->toBe('runtime_forwarder');

    $page = $this->actingAs($user)->get(route('settings.integrations.edit'));

    $page->assertInertia(fn (Assert $inertia) => $inertia
        ->component('settings/integrations')
        ->where('integrations.resend.connected', true)
        ->where('deliveryProjection.resend.provider', 'resend')
        ->where('deliveryProjection.resend.integrationId', 'resend.primary')
        ->where('deliveryProjection.resend.lastState', 'delivered')
        ->where('deliveryProjection.resend.lastMessageId', 'email_ownership_flow')
        ->where('deliveryProjection.resend.source', 'runtime_forwarder')
    );

    $before = $projection->updated_at?->toISOString();

    $this
        ->actingAs($user)
        ->post(route('settings.integrations.resend.upsert'), [
            'resend_api_key' => 're_test_intent_rotated_0987654321',
            'sender_email' => 'noreply@example.com',
            'sender_name' => 'OpenAgents',
        ])
        ->assertSessionHasNoErrors();

    $projection->refresh();

    expect((string) $projection->last_state)->toBe('delivered');
    expect((string) $projection->last_message_id)->toBe('email_ownership_flow');
    expect($projection->updated_at?->toISOString())->toBe($before);
});

/**
 * @param  array<string, mixed>  $overrides
 * @return array<string, mixed>
 */
function baseOwnershipPayload(string $type, array $overrides = []): array
{
    $base = [
        'type' => $type,
        'created_at' => '2026-02-19T18:00:00Z',
        'data' => [
            'email_id' => 'email_ownership',
            'to' => ['user@example.com'],
            'reason' => null,
            'tags' => [],
        ],
    ];

    return array_replace_recursive($base, $overrides);
}

/**
 * @param  array<string, string>  $overrides
 * @return array<string, string>
 */
function signedOwnershipWebhookHeaders(string $payload, array $overrides = []): array
{
    $id = $overrides['id'] ?? ('evt_'.substr(hash('sha256', $payload), 0, 20));
    $timestamp = $overrides['timestamp'] ?? (string) now()->unix();

    $secretConfig = (string) config('runtime.comms.resend.webhook_secret', '');
    $secretBytes = $secretConfig;

    if (str_starts_with($secretConfig, 'whsec_')) {
        $encoded = substr($secretConfig, strlen('whsec_'));
        $decoded = base64_decode($encoded, true);
        $secretBytes = $decoded === false ? $encoded : $decoded;
    }

    $signedContent = sprintf('%s.%s.%s', $id, $timestamp, $payload);
    $signature = base64_encode(hash_hmac('sha256', $signedContent, $secretBytes, true));

    return [
        'svix-id' => $id,
        'svix-timestamp' => $timestamp,
        'svix-signature' => $overrides['signature'] ?? ('v1,'.$signature),
    ];
}

/**
 * @param  array<string, string>  $headers
 */
function callOwnershipWebhook(Tests\TestCase $testCase, string $payload, array $headers)
{
    return $testCase->call(
        'POST',
        '/api/webhooks/resend',
        [],
        [],
        [],
        ownershipWebhookServerHeaders($headers),
        $payload,
    );
}

/**
 * @param  array<string, string>  $headers
 * @return array<string, string>
 */
function ownershipWebhookServerHeaders(array $headers): array
{
    $server = [
        'CONTENT_TYPE' => 'application/json',
    ];

    foreach ($headers as $name => $value) {
        $server['HTTP_'.strtoupper(str_replace('-', '_', $name))] = $value;
    }

    return $server;
}
