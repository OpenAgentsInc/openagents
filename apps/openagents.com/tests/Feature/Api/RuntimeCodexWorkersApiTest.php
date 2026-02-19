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
    config()->set('runtime.elixir.codex_worker_stop_path_template', '/internal/v1/codex/workers/{worker_id}/stop');
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

    $stop = $this->actingAs($user)->postJson('/api/runtime/codex/workers/codexw_1/stop', ['reason' => 'done']);
    $stop->assertStatus(202)->assertJsonPath('data.status', 'stopped');

    Http::assertSent(function (HttpRequest $request) use ($user): bool {
        return str_starts_with($request->url(), 'http://runtime.internal/internal/v1/codex/workers')
            && ($request->header('X-OA-USER-ID')[0] ?? null) === (string) $user->id;
    });
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
