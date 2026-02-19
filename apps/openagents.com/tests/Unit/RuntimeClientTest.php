<?php

use App\AI\Runtime\ElixirRuntimeClient;
use App\AI\Runtime\LegacyRuntimeClient;
use App\AI\Runtime\RuntimeClient;
use Illuminate\Contracts\Auth\Authenticatable;
use Illuminate\Http\Client\Request;
use Illuminate\Support\Facades\Http;

uses(Tests\TestCase::class);

test('runtime client binding resolves legacy driver', function () {
    config()->set('runtime.driver', 'legacy');

    $client = resolve(RuntimeClient::class);

    expect($client)->toBeInstanceOf(LegacyRuntimeClient::class);
    expect($client->driverName())->toBe('legacy');
});

test('runtime client binding resolves elixir driver', function () {
    config()->set('runtime.driver', 'elixir');
    config()->set('runtime.elixir.base_url', 'http://runtime.internal');
    config()->set('runtime.elixir.signing_key', 'test_signing_key');

    $client = resolve(RuntimeClient::class);

    expect($client)->toBeInstanceOf(ElixirRuntimeClient::class);
    expect($client->driverName())->toBe('elixir');
});

test('elixir runtime client streams upstream sse and signs internal request with bounded retries', function () {
    config()->set('runtime.elixir.base_url', 'http://runtime.internal');
    config()->set('runtime.elixir.stream_path', '/internal/v1/runs/stream');
    config()->set('runtime.elixir.signing_key', 'test_signing_key');
    config()->set('runtime.elixir.max_retries', 2);
    config()->set('runtime.elixir.retry_backoff_ms', 1);
    config()->set('runtime.elixir.timeout_ms', 5_000);
    config()->set('runtime.elixir.connect_timeout_ms', 1_000);

    Http::fakeSequence()
        ->push('temporarily unavailable', 502)
        ->push("data: {\"type\":\"start\"}\n\ndata: [DONE]\n\n", 200, [
            'Content-Type' => 'text/event-stream',
        ]);

    $client = resolve(ElixirRuntimeClient::class);
    $response = $client->streamAutopilotRun(fakeRuntimeUser(), 'thread_123', 'hello runtime');

    ob_start();
    $response->sendContent();
    $content = (string) ob_get_clean();

    expect($content)->toContain('data: {"type":"start"}');
    expect($content)->toContain("data: [DONE]\n\n");

    Http::assertSentCount(2);
    Http::assertSent(function (Request $request): bool {
        return $request->url() === 'http://runtime.internal/internal/v1/runs/stream'
            && $request->hasHeader('X-OA-RUNTIME-SIGNATURE')
            && $request->hasHeader('X-OA-RUNTIME-BODY-SHA256')
            && $request->hasHeader('X-OA-RUNTIME-KEY-ID')
            && $request['threadId'] === 'thread_123'
            && $request['prompt'] === 'hello runtime'
            && $request['userId'] === 42;
    });
});

test('elixir runtime client emits fallback sse when upstream stays unavailable', function () {
    config()->set('runtime.elixir.base_url', 'http://runtime.internal');
    config()->set('runtime.elixir.stream_path', '/internal/v1/runs/stream');
    config()->set('runtime.elixir.signing_key', 'test_signing_key');
    config()->set('runtime.elixir.max_retries', 1);
    config()->set('runtime.elixir.retry_backoff_ms', 1);

    Http::fakeSequence()
        ->push('bad gateway', 502)
        ->push('still bad', 503);

    $client = resolve(ElixirRuntimeClient::class);
    $response = $client->streamAutopilotRun(fakeRuntimeUser(), 'thread_456', 'fallback prompt');

    ob_start();
    $response->sendContent();
    $content = (string) ob_get_clean();

    expect($content)->toContain('runtime_fallback');
    expect($content)->toContain('The runtime is unavailable. Please try again.');
    expect($content)->toContain("data: [DONE]\n\n");

    Http::assertSentCount(2);
});

function fakeRuntimeUser(): Authenticatable
{
    return new class implements Authenticatable
    {
        public string $email = 'runtime-user@example.com';

        public function getAuthIdentifierName(): string
        {
            return 'id';
        }

        public function getAuthIdentifier(): int
        {
            return 42;
        }

        public function getAuthPassword(): string
        {
            return '';
        }

        public function getAuthPasswordName(): string
        {
            return 'password';
        }

        public function getRememberToken(): ?string
        {
            return null;
        }

        public function setRememberToken($value): void {}

        public function getRememberTokenName(): string
        {
            return 'remember_token';
        }
    };
}
