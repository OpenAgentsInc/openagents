<?php

use App\Jobs\ForwardResendWebhookToRuntime;
use App\Models\User;
use App\Models\UserIntegration;
use App\Models\UserIntegrationAudit;
use App\Support\Comms\RuntimeCommsDeliveryForwarder;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Queue;

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

test('resend webhook rejects invalid signatures and audits attempt', function () {
    $payload = json_encode(baseResendPayload('email.delivered'), JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
    expect($payload)->toBeString();

    $headers = signedResendWebhookHeaders((string) $payload, ['signature' => 'v1,invalid']);

    $response = callResendWebhook($this, (string) $payload, $headers);

    $response
        ->assertStatus(401)
        ->assertJsonPath('error.code', 'invalid_signature');

    $row = DB::table('comms_webhook_events')->latest('id')->first();
    expect($row)->not->toBeNull();
    expect((string) $row->status)->toBe('invalid_signature');
    expect((bool) $row->signature_valid)->toBeFalse();
});

test('resend webhook normalizes delivery states and deduplicates retries by svix-id', function () {
    Queue::fake();

    $types = [
        'email.delivered' => 'delivered',
        'email.bounced' => 'bounced',
        'email.complained' => 'complained',
        'email.suppressed' => 'unsubscribed',
    ];

    foreach ($types as $type => $expectedState) {
        $payloadArray = baseResendPayload($type);
        $payloadArray['data']['email_id'] = 'email_'.$expectedState;

        $payload = json_encode($payloadArray, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
        expect($payload)->toBeString();

        $headers = signedResendWebhookHeaders((string) $payload, ['id' => 'evt_'.$expectedState]);

        $first = callResendWebhook($this, (string) $payload, $headers);
        $first
            ->assertStatus(202)
            ->assertJsonPath('data.status', 'received')
            ->assertJsonPath('data.idempotent_replay', false);

        $second = callResendWebhook($this, (string) $payload, $headers);
        $second
            ->assertStatus(200)
            ->assertJsonPath('data.idempotent_replay', true);

        $row = DB::table('comms_webhook_events')
            ->where('external_event_id', 'evt_'.$expectedState)
            ->first();

        expect($row)->not->toBeNull();
        expect((string) $row->delivery_state)->toBe($expectedState);
        expect((string) $row->status)->toBe('received');
    }

    $count = DB::table('comms_webhook_events')->where('provider', 'resend')->count();
    expect($count)->toBe(4);

    Queue::assertPushed(ForwardResendWebhookToRuntime::class, 8);
});

test('resend webhook returns idempotency conflict when payload changes for same svix-id', function () {
    Queue::fake();

    $delivered = json_encode(baseResendPayload('email.delivered'), JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
    $bounced = json_encode(baseResendPayload('email.bounced'), JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);

    expect($delivered)->toBeString();
    expect($bounced)->toBeString();

    $headers = signedResendWebhookHeaders((string) $delivered, ['id' => 'evt_conflict']);

    callResendWebhook($this, (string) $delivered, $headers)->assertStatus(202);

    $conflictingHeaders = signedResendWebhookHeaders((string) $bounced, ['id' => 'evt_conflict']);

    callResendWebhook($this, (string) $bounced, $conflictingHeaders)
        ->assertStatus(409)
        ->assertJsonPath('error.code', 'idempotency_conflict');
});

test('forward job retries after runtime failure and updates projection on success', function () {
    Queue::fake();

    $user = User::factory()->create();

    UserIntegration::query()->create([
        'user_id' => $user->id,
        'provider' => 'resend',
        'status' => 'active',
        'encrypted_secret' => 're_test_1234567890',
        'secret_fingerprint' => hash('sha256', 're_test_1234567890'),
        'secret_last4' => '7890',
        'metadata' => [
            'sender_email' => 'noreply@example.com',
            'sender_name' => 'OpenAgents',
        ],
        'connected_at' => now(),
    ]);

    $payloadArray = baseResendPayload('email.delivered');
    $payloadArray['data']['tags'][] = ['name' => 'user_id', 'value' => (string) $user->id];
    $payloadArray['data']['tags'][] = ['name' => 'integration_id', 'value' => 'resend.primary'];
    $payloadArray['data']['email_id'] = 'email_projection';

    $payload = json_encode($payloadArray, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
    expect($payload)->toBeString();

    $headers = signedResendWebhookHeaders((string) $payload, ['id' => 'evt_projection']);

    callResendWebhook($this, (string) $payload, $headers)->assertStatus(202);

    $queuedJob = null;

    Queue::assertPushed(ForwardResendWebhookToRuntime::class, function (ForwardResendWebhookToRuntime $job) use (&$queuedJob): bool {
        $queuedJob = $job;

        return true;
    });

    expect($queuedJob)->toBeInstanceOf(ForwardResendWebhookToRuntime::class);

    Http::fakeSequence()
        ->push(['error' => 'temporary'], 500)
        ->push(['eventId' => 'evt_projection', 'status' => 'accepted', 'idempotentReplay' => false], 202);

    $forwarder = app(RuntimeCommsDeliveryForwarder::class);

    expect(fn () => $queuedJob->handle($forwarder))->toThrow(RuntimeException::class);

    $failedRow = DB::table('comms_webhook_events')->where('external_event_id', 'evt_projection')->first();
    expect($failedRow)->not->toBeNull();
    expect((string) $failedRow->status)->toBe('failed');
    expect((int) $failedRow->runtime_attempts)->toBe(1);

    $queuedJob->handle($forwarder);

    $forwardedRow = DB::table('comms_webhook_events')->where('external_event_id', 'evt_projection')->first();
    expect((string) $forwardedRow->status)->toBe('forwarded');
    expect((int) $forwardedRow->runtime_attempts)->toBe(2);
    expect((int) $forwardedRow->runtime_status_code)->toBe(202);
    expect($forwardedRow->forwarded_at)->not->toBeNull();

    $integration = UserIntegration::query()->where('user_id', $user->id)->where('provider', 'resend')->first();
    expect($integration)->not->toBeNull();

    $projection = $integration->metadata['delivery_projection'] ?? [];
    expect((string) ($projection['last_state'] ?? ''))->toBe('delivered');
    expect((string) ($projection['last_message_id'] ?? ''))->toBe('email_projection');

    $audit = UserIntegrationAudit::query()
        ->where('user_id', $user->id)
        ->where('provider', 'resend')
        ->where('action', 'delivery_projection_updated')
        ->latest('id')
        ->first();

    expect($audit)->not->toBeNull();
});

/**
 * @param  array<string, mixed>  $overrides
 * @return array<string, mixed>
 */
function baseResendPayload(string $type, array $overrides = []): array
{
    $base = [
        'type' => $type,
        'created_at' => '2026-02-19T18:00:00Z',
        'data' => [
            'email_id' => 'email_123',
            'to' => ['user@example.com'],
            'reason' => $type === 'email.bounced' ? 'mailbox_not_found' : null,
            'tags' => [
                ['name' => 'integration_id', 'value' => 'resend.primary'],
            ],
        ],
    ];

    return array_replace_recursive($base, $overrides);
}

/**
 * @param  array<string, string>  $overrides
 * @return array<string, string>
 */
function signedResendWebhookHeaders(string $payload, array $overrides = []): array
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
function callResendWebhook(Tests\TestCase $testCase, string $payload, array $headers)
{
    return $testCase->call(
        'POST',
        '/api/webhooks/resend',
        [],
        [],
        [],
        webhookServerHeaders($headers),
        $payload,
    );
}

/**
 * @param  array<string, string>  $headers
 * @return array<string, string>
 */
function webhookServerHeaders(array $headers): array
{
    $server = [
        'CONTENT_TYPE' => 'application/json',
    ];

    foreach ($headers as $name => $value) {
        $server['HTTP_'.strtoupper(str_replace('-', '_', $name))] = $value;
    }

    return $server;
}
