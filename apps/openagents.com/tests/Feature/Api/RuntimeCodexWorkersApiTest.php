<?php

use App\Models\User;
use Illuminate\Http\Client\Request as HttpRequest;
use Illuminate\Support\Facades\Http;

beforeEach(function () {
    config()->set('runtime.elixir.base_url', 'http://runtime.internal');
    config()->set('runtime.elixir.signing_key', 'runtime-signing-key');
    config()->set('runtime.elixir.signing_key_id', 'runtime-v1');
    config()->set('runtime.elixir.signature_ttl_seconds', 60);
    config()->set('runtime.elixir.max_retries', 0);
    config()->set('runtime.elixir.retry_backoff_ms', 1);
    config()->set('runtime.elixir.codex_workers_path', '/internal/v1/codex/workers');
    config()->set('runtime.elixir.codex_worker_snapshot_path_template', '/internal/v1/codex/workers/{worker_id}/snapshot');
    config()->set('runtime.elixir.codex_worker_stream_path_template', '/internal/v1/codex/workers/{worker_id}/stream');
    config()->set('runtime.elixir.codex_worker_requests_path_template', '/internal/v1/codex/workers/{worker_id}/requests');
    config()->set('runtime.elixir.codex_worker_events_path_template', '/internal/v1/codex/workers/{worker_id}/events');
    config()->set('runtime.elixir.codex_worker_stop_path_template', '/internal/v1/codex/workers/{worker_id}/stop');
});

test('runtime codex workers api lists principal-owned workers with filters', function () {
    $user = User::factory()->create();

    Http::fake([
        'http://runtime.internal/internal/v1/codex/workers?status=running&limit=5' => Http::response([
            'data' => [
                [
                    'worker_id' => 'codexw_1',
                    'status' => 'running',
                    'latest_seq' => 3,
                    'khala_projection' => [
                        'status' => 'in_sync',
                        'lag_events' => 0,
                    ],
                ],
            ],
        ], 200),
    ]);

    $response = $this->actingAs($user)->getJson('/api/runtime/codex/workers?status=running&limit=5');
    $response
        ->assertOk()
        ->assertJsonPath('data.0.worker_id', 'codexw_1')
        ->assertJsonPath('data.0.khala_projection.status', 'in_sync');

    Http::assertSent(function (HttpRequest $request) use ($user): bool {
        return $request->url() === 'http://runtime.internal/internal/v1/codex/workers?status=running&limit=5'
            && ($request->header('X-OA-USER-ID')[0] ?? null) === (string) $user->id;
    });
});

test('runtime codex workers api forwards trace headers for correlation', function () {
    $user = User::factory()->create();
    $traceparent = '00-11111111111111111111111111111111-2222222222222222-01';
    $tracestate = 'vendorname=opaque';
    $requestId = 'req-codex-correlation';

    Http::fake([
        'http://runtime.internal/internal/v1/codex/workers?limit=1' => Http::response([
            'data' => [],
        ], 200),
    ]);

    $this->actingAs($user)
        ->withHeaders([
            'traceparent' => $traceparent,
            'tracestate' => $tracestate,
            'x-request-id' => $requestId,
        ])
        ->getJson('/api/runtime/codex/workers?limit=1')
        ->assertOk();

    Http::assertSentCount(1);
    /** @var array{0: HttpRequest, 1: mixed} $recorded */
    $recorded = Http::recorded()->first();
    $forwarded = $recorded[0];

    $normalizedHeaders = [];
    foreach ($forwarded->headers() as $name => $values) {
        $normalizedHeaders[strtolower((string) $name)] = $values;
    }

    expect($forwarded->url())->toBe('http://runtime.internal/internal/v1/codex/workers?limit=1');
    expect($normalizedHeaders['traceparent'][0] ?? null)->toBe($traceparent);
    expect($normalizedHeaders['tracestate'][0] ?? null)->toBe($tracestate);
    expect($normalizedHeaders['x-request-id'][0] ?? null)->toBe($requestId);
});

test('runtime codex workers api proxies lifecycle endpoints', function () {
    $user = User::factory()->create();

    Http::fake([
        'http://runtime.internal/internal/v1/codex/workers' => Http::response([
            'data' => ['workerId' => 'codexw_1', 'status' => 'running', 'idempotentReplay' => false],
        ], 202),
        'http://runtime.internal/internal/v1/codex/workers/codexw_1/snapshot' => Http::response([
            'data' => ['worker_id' => 'codexw_1', 'status' => 'running'],
        ], 200),
        'http://runtime.internal/internal/v1/codex/workers/codexw_1/requests' => Http::response([
            'data' => ['worker_id' => 'codexw_1', 'ok' => true],
        ], 200),
        'http://runtime.internal/internal/v1/codex/workers/codexw_1/events' => Http::response([
            'data' => ['worker_id' => 'codexw_1', 'seq' => 4, 'event_type' => 'worker.event'],
        ], 202),
        'http://runtime.internal/internal/v1/codex/workers/codexw_1/stop' => Http::response([
            'data' => ['worker_id' => 'codexw_1', 'status' => 'stopped', 'idempotent_replay' => false],
        ], 202),
    ]);

    $create = $this->actingAs($user)->postJson('/api/runtime/codex/workers', [
        'worker_id' => 'codexw_1',
        'workspace_ref' => 'workspace://demo',
    ]);
    $create->assertStatus(202)->assertJsonPath('data.workerId', 'codexw_1');

    $show = $this->actingAs($user)->getJson('/api/runtime/codex/workers/codexw_1');
    $show->assertOk()->assertJsonPath('data.worker_id', 'codexw_1');

    $request = $this->actingAs($user)->postJson('/api/runtime/codex/workers/codexw_1/requests', [
        'request' => [
            'request_id' => 'req_1',
            'method' => 'thread/start',
            'params' => ['prompt' => 'hello'],
        ],
    ]);
    $request->assertOk()->assertJsonPath('data.ok', true);

    $events = $this->actingAs($user)->postJson('/api/runtime/codex/workers/codexw_1/events', [
        'event' => [
            'event_type' => 'worker.event',
            'payload' => [
                'source' => 'desktop',
                'method' => 'turn/started',
            ],
        ],
    ]);
    $events->assertStatus(202)->assertJsonPath('data.seq', 4);

    $stop = $this->actingAs($user)->postJson('/api/runtime/codex/workers/codexw_1/stop', ['reason' => 'done']);
    $stop->assertStatus(202)->assertJsonPath('data.status', 'stopped');

    Http::assertSent(function (HttpRequest $request) use ($user): bool {
        return str_starts_with($request->url(), 'http://runtime.internal/internal/v1/codex/workers')
            && ($request->header('X-OA-USER-ID')[0] ?? null) === (string) $user->id;
    });
});

test('runtime codex workers api passes through runtime conflict for stopped worker mutation', function () {
    $user = User::factory()->create();

    Http::fake([
        'http://runtime.internal/internal/v1/codex/workers/codexw_2/events' => Http::response([
            'error' => ['code' => 'conflict', 'message' => 'worker is stopped; create or reattach to resume'],
        ], 409),
    ]);

    $response = $this->actingAs($user)->postJson('/api/runtime/codex/workers/codexw_2/events', [
        'event' => [
            'event_type' => 'worker.event',
            'payload' => ['source' => 'desktop', 'method' => 'turn/started'],
        ],
    ]);

    $response->assertStatus(409)->assertJsonPath('error.code', 'conflict');
});

test('runtime codex workers api proxies ios handshake ingest payload shape', function () {
    $user = User::factory()->create();
    $handshakeId = 'hs_'.random_int(1000, 9999);

    Http::fake([
        'http://runtime.internal/internal/v1/codex/workers/codexw_hs/events' => Http::response([
            'data' => ['worker_id' => 'codexw_hs', 'seq' => 8, 'event_type' => 'worker.event'],
        ], 202),
    ]);

    $response = $this->actingAs($user)->postJson('/api/runtime/codex/workers/codexw_hs/events', [
        'event' => [
            'event_type' => 'worker.event',
            'payload' => [
                'source' => 'autopilot-ios',
                'method' => 'ios/handshake',
                'handshake_id' => $handshakeId,
                'device_id' => 'device_test',
                'occurred_at' => '2026-02-20T00:00:00Z',
            ],
        ],
    ]);

    $response
        ->assertStatus(202)
        ->assertJsonPath('data.worker_id', 'codexw_hs')
        ->assertJsonPath('data.seq', 8);

    Http::assertSent(function (HttpRequest $request) use ($user, $handshakeId): bool {
        $data = $request->data();

        return $request->url() === 'http://runtime.internal/internal/v1/codex/workers/codexw_hs/events'
            && ($request->header('X-OA-USER-ID')[0] ?? null) === (string) $user->id
            && ($data['event']['event_type'] ?? null) === 'worker.event'
            && ($data['event']['payload']['source'] ?? null) === 'autopilot-ios'
            && ($data['event']['payload']['method'] ?? null) === 'ios/handshake'
            && ($data['event']['payload']['handshake_id'] ?? null) === $handshakeId
            && ($data['event']['payload']['device_id'] ?? null) === 'device_test'
            && ($data['event']['payload']['occurred_at'] ?? null) === '2026-02-20T00:00:00Z';
    });
});

test('runtime codex workers api rejects unauthenticated handshake ingest', function () {
    Http::fake();

    $response = $this->postJson('/api/runtime/codex/workers/codexw_auth/events', [
        'event' => [
            'event_type' => 'worker.event',
            'payload' => [
                'source' => 'autopilot-ios',
                'method' => 'ios/handshake',
                'handshake_id' => 'hs_unauth',
                'device_id' => 'device_test',
                'occurred_at' => '2026-02-20T00:00:00Z',
            ],
        ],
    ]);

    $response->assertUnauthorized();
    Http::assertNothingSent();
});

test('runtime codex workers api passes through handshake ingest conflict errors', function () {
    $user = User::factory()->create();

    Http::fake([
        'http://runtime.internal/internal/v1/codex/workers/codexw_hs_conflict/events' => Http::response([
            'error' => ['code' => 'conflict', 'message' => 'handshake already acknowledged'],
        ], 409),
    ]);

    $response = $this->actingAs($user)->postJson('/api/runtime/codex/workers/codexw_hs_conflict/events', [
        'event' => [
            'event_type' => 'worker.event',
            'payload' => [
                'source' => 'autopilot-ios',
                'method' => 'ios/handshake',
                'handshake_id' => 'hs_conflict',
                'device_id' => 'device_test',
                'occurred_at' => '2026-02-20T00:00:00Z',
            ],
        ],
    ]);

    $response
        ->assertStatus(409)
        ->assertJsonPath('error.code', 'conflict')
        ->assertJsonPath('error.message', 'handshake already acknowledged');
});

test('runtime codex workers api proxies stream endpoint with cursor and last-event-id semantics', function () {
    $user = User::factory()->create();

    Http::fake([
        'http://runtime.internal/internal/v1/codex/workers/codexw_stream/snapshot' => Http::response([
            'data' => ['worker_id' => 'codexw_stream', 'status' => 'running'],
        ], 200),
        'http://runtime.internal/internal/v1/codex/workers/codexw_stream/stream*' => Http::response(
            "event: message\nid: 3\ndata: {\"worker_id\":\"codexw_stream\",\"type\":\"worker.response\"}\n\n",
            200,
            ['Content-Type' => 'text/event-stream']
        ),
    ]);

    $response = $this->actingAs($user)
        ->withHeaders(['Last-Event-ID' => '2'])
        ->get('/api/runtime/codex/workers/codexw_stream/stream?cursor=2&tail_ms=15000');

    $response->assertOk();
    expect((string) $response->headers->get('content-type'))->toContain('text/event-stream');

    $streamBody = $response->streamedContent();
    expect($streamBody)->toContain('id: 3');
    expect($streamBody)->toContain('worker.response');

    Http::assertSent(function (HttpRequest $request) use ($user): bool {
        return $request->url() === 'http://runtime.internal/internal/v1/codex/workers/codexw_stream/snapshot'
            && ($request->header('X-OA-USER-ID')[0] ?? null) === (string) $user->id;
    });

    Http::assertSent(function (HttpRequest $request) use ($user): bool {
        return $request->url() === 'http://runtime.internal/internal/v1/codex/workers/codexw_stream/stream?cursor=2&tail_ms=15000'
            && ($request->header('X-OA-USER-ID')[0] ?? null) === (string) $user->id
            && ($request->header('Last-Event-ID')[0] ?? null) === '2';
    });
});

test('runtime codex worker stream proxy denies unauthorized ownership before opening stream', function () {
    $user = User::factory()->create();

    Http::fake([
        'http://runtime.internal/internal/v1/codex/workers/codexw_forbidden/snapshot' => Http::response([
            'error' => ['code' => 'forbidden', 'message' => 'forbidden'],
        ], 403),
        'http://runtime.internal/internal/v1/codex/workers/codexw_forbidden/stream*' => Http::response(
            "event: message\nid: 9\ndata: should-not-open\n\n",
            200,
            ['Content-Type' => 'text/event-stream']
        ),
    ]);

    $this->actingAs($user)
        ->getJson('/api/runtime/codex/workers/codexw_forbidden/stream')
        ->assertStatus(403)
        ->assertJsonPath('error.code', 'forbidden');

    Http::assertSentCount(1);
});
