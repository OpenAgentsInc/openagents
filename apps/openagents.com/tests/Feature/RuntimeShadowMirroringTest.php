<?php

use App\AI\Runtime\RuntimeClient;
use App\AI\Runtime\ShadowRuntimeClient;
use App\Models\User;
use Illuminate\Support\Facades\DB;
use Symfony\Component\HttpFoundation\StreamedResponse;

test('shadow runtime client mirrors streams and persists semantic diff records', function () {
    config()->set('runtime.shadow.sample_rate', 1.0);
    config()->set('runtime.shadow.max_capture_bytes', 50_000);

    $user = User::factory()->create();

    $primary = fakeSseRuntimeClient('legacy', "data: {\"type\":\"start\"}\n\ndata: {\"type\":\"text-delta\",\"id\":\"a\",\"delta\":\"legacy\"}\n\ndata: {\"type\":\"finish\"}\n\ndata: [DONE]\n\n");
    $shadow = fakeSseRuntimeClient('elixir', "data: {\"type\":\"start\"}\n\ndata: {\"type\":\"text-delta\",\"id\":\"a\",\"delta\":\"shadow\"}\n\ndata: {\"type\":\"finish\"}\n\ndata: [DONE]\n\n");

    $client = new ShadowRuntimeClient($primary, $shadow);

    $response = $client->streamAutopilotRun($user, 'thread_shadow_1', 'hello shadow');

    ob_start();
    $response->sendContent();
    $content = (string) ob_get_clean();

    expect($content)->toContain('legacy');

    $record = DB::table('runtime_shadow_diffs')->latest('created_at')->first();

    expect($record)->not->toBeNull();
    expect($record->status)->toBe('compared');

    $diff = json_decode((string) $record->diff, true);
    expect($diff['pass'] ?? false)->toBeTrue();
});

test('shadow runtime client records mismatch details when semantic invariants diverge', function () {
    config()->set('runtime.shadow.sample_rate', 1.0);

    $user = User::factory()->create();

    $primary = fakeSseRuntimeClient('legacy', "data: {\"type\":\"start\"}\n\ndata: {\"type\":\"text-delta\",\"id\":\"a\",\"delta\":\"legacy\"}\n\ndata: {\"type\":\"finish\"}\n\ndata: [DONE]\n\n");
    $shadow = fakeSseRuntimeClient('elixir', "data: {\"type\":\"start\"}\n\ndata: {\"type\":\"text-delta\",\"id\":\"a\",\"delta\":\"shadow\"}\n\n");

    $client = new ShadowRuntimeClient($primary, $shadow);

    ob_start();
    $client->streamAutopilotRun($user, 'thread_shadow_2', 'hello shadow mismatch')->sendContent();
    ob_end_clean();

    $record = DB::table('runtime_shadow_diffs')->latest('created_at')->first();
    $diff = json_decode((string) $record->diff, true);

    expect($record->status)->toBe('compared');
    expect($diff['pass'] ?? true)->toBeFalse();
    expect($diff['mismatches'] ?? [])->toContain('has_finish');
    expect($diff['mismatches'] ?? [])->toContain('has_done');
});

test('shadow runtime client respects sampling and skips persistence when sample rate is zero', function () {
    config()->set('runtime.shadow.sample_rate', 0.0);

    $user = User::factory()->create();

    $primary = fakeSseRuntimeClient('legacy', "data: {\"type\":\"start\"}\n\ndata: [DONE]\n\n");
    $shadow = fakeSseRuntimeClient('elixir', "data: {\"type\":\"start\"}\n\ndata: [DONE]\n\n");

    $client = new ShadowRuntimeClient($primary, $shadow);

    ob_start();
    $client->streamAutopilotRun($user, 'thread_shadow_3', 'no mirror')->sendContent();
    ob_end_clean();

    expect(DB::table('runtime_shadow_diffs')->count())->toBe(0);
});

function fakeSseRuntimeClient(string $driverName, string $payload): RuntimeClient
{
    return new class($driverName, $payload) implements RuntimeClient
    {
        public function __construct(
            private readonly string $driverNameValue,
            private readonly string $payload,
        ) {}

        public function driverName(): string
        {
            return $this->driverNameValue;
        }

        public function streamAutopilotRun(
            \Illuminate\Contracts\Auth\Authenticatable $user,
            string $threadId,
            string $prompt,
            bool $authenticatedSession = true,
        ): StreamedResponse {
            return response()->stream(function (): void {
                echo $this->payload;
            }, 200, [
                'Content-Type' => 'text/event-stream',
                'x-vercel-ai-ui-message-stream' => 'v1',
            ]);
        }
    };
}
