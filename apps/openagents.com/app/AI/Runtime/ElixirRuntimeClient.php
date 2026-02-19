<?php

namespace App\AI\Runtime;

use Illuminate\Contracts\Auth\Authenticatable;
use Illuminate\Http\Client\Response as HttpResponse;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Str;
use RuntimeException;
use Symfony\Component\HttpFoundation\StreamedResponse;
use Throwable;

final class ElixirRuntimeClient implements RuntimeClient
{
    public function driverName(): string
    {
        return 'elixir';
    }

    public function streamAutopilotRun(
        Authenticatable $user,
        string $threadId,
        string $prompt,
        bool $authenticatedSession = true,
    ): StreamedResponse {
        $payload = $this->buildPayload($user, $threadId, $prompt, $authenticatedSession);
        $shouldFlush = ! app()->runningUnitTests();

        return response()->stream(function () use ($payload, $shouldFlush): void {
            try {
                $response = $this->openStream($payload);
                $body = $response->toPsrResponse()->getBody();
                $chunkBytes = max(128, (int) config('runtime.elixir.stream_chunk_bytes', 1024));

                while (! $body->eof()) {
                    $chunk = $body->read($chunkBytes);

                    if ($chunk === '') {
                        usleep(10_000);

                        continue;
                    }

                    echo $chunk;
                    $this->flushOutput($shouldFlush);
                }
            } catch (Throwable $e) {
                Log::error('Elixir runtime stream failed', [
                    'error' => $e->getMessage(),
                    'code' => $e->getCode(),
                ]);

                $this->emitFallbackStream($shouldFlush);
            }
        }, 200, [
            'Content-Type' => 'text/event-stream',
            'Cache-Control' => 'no-cache, no-transform',
            'x-vercel-ai-ui-message-stream' => 'v1',
            'X-Accel-Buffering' => 'no',
        ]);
    }

    /**
     * @param  array<string, mixed>  $payload
     */
    private function openStream(array $payload): HttpResponse
    {
        $url = $this->streamUrl();
        $maxRetries = max(0, (int) config('runtime.elixir.max_retries', 2));
        $attempts = $maxRetries + 1;
        $backoffMs = max(0, (int) config('runtime.elixir.retry_backoff_ms', 200));
        $connectTimeoutSeconds = max(1, (int) ceil(((int) config('runtime.elixir.connect_timeout_ms', 2500)) / 1000));
        $timeoutSeconds = max(1, (int) ceil(((int) config('runtime.elixir.timeout_ms', 60000)) / 1000));

        $lastException = null;

        for ($attempt = 1; $attempt <= $attempts; $attempt++) {
            try {
                $response = Http::withHeaders($this->signedHeaders($payload))
                    ->accept('text/event-stream')
                    ->connectTimeout($connectTimeoutSeconds)
                    ->timeout($timeoutSeconds)
                    ->withOptions(['stream' => true])
                    ->post($url, $payload);

                if ($response->successful()) {
                    return $response;
                }

                $lastException = new RuntimeException(sprintf(
                    'Elixir runtime returned HTTP %d',
                    $response->status()
                ));
            } catch (Throwable $e) {
                $lastException = $e;
            }

            if ($attempt < $attempts && $backoffMs > 0) {
                usleep($backoffMs * 1000);
            }
        }

        throw $lastException instanceof Throwable
            ? $lastException
            : new RuntimeException('Elixir runtime stream request failed');
    }

    /**
     * @param  array<string, mixed>  $payload
     * @return array<string, string>
     */
    private function signedHeaders(array $payload): array
    {
        $timestamp = (string) now()->unix();
        $nonce = (string) Str::uuid();
        $payloadJson = json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
        $payloadHash = hash('sha256', $payloadJson === false ? '{}' : $payloadJson);
        $key = (string) config('runtime.elixir.signing_key', '');
        $signature = hash_hmac('sha256', implode("\n", [$timestamp, $nonce, $payloadHash]), $key);

        $headers = [
            'X-OA-RUNTIME-TIMESTAMP' => $timestamp,
            'X-OA-RUNTIME-NONCE' => $nonce,
            'X-OA-RUNTIME-BODY-SHA256' => $payloadHash,
            'X-OA-RUNTIME-SIGNATURE' => $signature,
            'X-OA-RUNTIME-KEY-ID' => (string) config('runtime.elixir.signing_key_id', 'runtime-v1'),
            'X-OA-RUNTIME-SIGNATURE-TTL' => (string) config('runtime.elixir.signature_ttl_seconds', 60),
            'X-Request-Id' => (string) Str::uuid(),
        ];

        $request = request();

        foreach (['traceparent', 'tracestate', 'x-request-id'] as $traceHeader) {
            $value = $request?->header($traceHeader);
            if (is_string($value) && $value !== '') {
                $headers[$traceHeader] = $value;
            }
        }

        return $headers;
    }

    private function streamUrl(): string
    {
        $baseUrl = (string) config('runtime.elixir.base_url', '');
        $path = (string) config('runtime.elixir.stream_path', '/internal/v1/runs/stream');

        if ($baseUrl === '') {
            throw new RuntimeException('runtime.elixir.base_url is not configured');
        }

        return rtrim($baseUrl, '/').'/'.ltrim($path, '/');
    }

    /**
     * @return array<string, mixed>
     */
    private function buildPayload(
        Authenticatable $user,
        string $threadId,
        string $prompt,
        bool $authenticatedSession,
    ): array {
        return [
            'threadId' => $threadId,
            'prompt' => $prompt,
            'userId' => (int) $user->getAuthIdentifier(),
            'userEmail' => (string) ($user->email ?? ''),
            'authenticatedSession' => $authenticatedSession,
        ];
    }

    private function emitFallbackStream(bool $shouldFlush): void
    {
        $events = [
            ['type' => 'start'],
            ['type' => 'start-step'],
            ['type' => 'text-start', 'id' => 'runtime_fallback'],
            ['type' => 'text-delta', 'id' => 'runtime_fallback', 'delta' => 'The runtime is unavailable. Please try again.'],
            ['type' => 'text-end', 'id' => 'runtime_fallback'],
            ['type' => 'finish'],
        ];

        foreach ($events as $event) {
            echo 'data: '.json_encode($event)."\n\n";
            $this->flushOutput($shouldFlush);
        }

        echo "data: [DONE]\n\n";
        $this->flushOutput($shouldFlush);
    }

    private function flushOutput(bool $shouldFlush): void
    {
        if (! $shouldFlush) {
            return;
        }

        if (ob_get_level() > 0) {
            ob_flush();
        }

        flush();
    }
}
