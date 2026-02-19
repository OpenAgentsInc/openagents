<?php

namespace App\AI\Runtime;

use Illuminate\Http\Client\Response;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Str;
use Symfony\Component\HttpFoundation\StreamedResponse;
use Throwable;

final class RuntimeCodexClient
{
    public function __construct(private readonly RuntimeSignatureTokenFactory $tokenFactory) {}

    /**
     * @param  array<string, mixed>|null  $payload
     * @param  array<string, mixed>  $contextClaims
     * @return array{ok: bool, status: int|null, body: mixed, error: string|null}
     */
    public function request(string $method, string $path, ?array $payload = null, array $contextClaims = []): array
    {
        $baseUrl = (string) config('runtime.elixir.base_url', '');
        $signingKey = (string) config('runtime.elixir.signing_key', '');

        if ($baseUrl === '' || $signingKey === '') {
            return [
                'ok' => false,
                'status' => null,
                'body' => null,
                'error' => 'runtime_codex_misconfigured',
            ];
        }

        $url = rtrim($baseUrl, '/').'/'.ltrim($path, '/');

        $maxRetries = max(0, (int) config('runtime.elixir.max_retries', 2));
        $attempts = $maxRetries + 1;
        $backoffMs = max(0, (int) config('runtime.elixir.retry_backoff_ms', 200));
        $timeoutSeconds = max(1, (int) ceil(((int) config('runtime.elixir.timeout_ms', 60000)) / 1000));
        $connectTimeoutSeconds = max(1, (int) ceil(((int) config('runtime.elixir.connect_timeout_ms', 2500)) / 1000));

        $lastStatus = null;
        $lastBody = null;
        $lastError = null;

        for ($attempt = 1; $attempt <= $attempts; $attempt++) {
            try {
                $request = Http::withHeaders($this->headers($payload, $contextClaims))
                    ->acceptJson()
                    ->connectTimeout($connectTimeoutSeconds)
                    ->timeout($timeoutSeconds);

                $response = match (strtoupper($method)) {
                    'GET' => $request->get($url),
                    'POST' => $request->post($url, $payload ?? []),
                    default => throw new \InvalidArgumentException('unsupported method'),
                };

                $lastStatus = $response->status();
                $lastBody = $this->normalizeResponseBody($response);

                if ($response->successful()) {
                    return [
                        'ok' => true,
                        'status' => $lastStatus,
                        'body' => $lastBody,
                        'error' => null,
                    ];
                }

                $lastError = sprintf('runtime_codex_http_%d', $lastStatus);
            } catch (Throwable $e) {
                $lastError = $e->getMessage();
            }

            if ($attempt < $attempts && $backoffMs > 0) {
                usleep($backoffMs * 1000);
            }
        }

        return [
            'ok' => false,
            'status' => $lastStatus,
            'body' => $lastBody,
            'error' => $lastError ?? 'runtime_codex_failed',
        ];
    }

    /**
     * @param  array<string, scalar>  $query
     * @param  array<string, mixed>  $contextClaims
     */
    public function stream(string $path, array $query = [], array $contextClaims = []): StreamedResponse
    {
        $baseUrl = (string) config('runtime.elixir.base_url', '');
        $signingKey = (string) config('runtime.elixir.signing_key', '');
        $shouldFlush = ! app()->runningUnitTests();

        if ($baseUrl === '' || $signingKey === '') {
            return response()->stream(function () use ($shouldFlush): void {
                $this->emitStreamError('runtime codex stream is not configured', $shouldFlush);
            }, 503, $this->streamHeaders());
        }

        $url = rtrim($baseUrl, '/').'/'.ltrim($path, '/');

        if ($query !== []) {
            $url .= '?'.http_build_query($query);
        }

        $maxRetries = max(0, (int) config('runtime.elixir.max_retries', 2));
        $attempts = $maxRetries + 1;
        $backoffMs = max(0, (int) config('runtime.elixir.retry_backoff_ms', 200));
        $timeoutSeconds = max(1, (int) ceil(((int) config('runtime.elixir.timeout_ms', 60000)) / 1000));
        $connectTimeoutSeconds = max(1, (int) ceil(((int) config('runtime.elixir.connect_timeout_ms', 2500)) / 1000));
        $chunkBytes = max(128, (int) config('runtime.elixir.stream_chunk_bytes', 1024));

        return response()->stream(function () use (
            $contextClaims,
            $url,
            $attempts,
            $backoffMs,
            $timeoutSeconds,
            $connectTimeoutSeconds,
            $chunkBytes,
            $shouldFlush
        ): void {
            $lastException = null;

            for ($attempt = 1; $attempt <= $attempts; $attempt++) {
                try {
                    $response = Http::withHeaders($this->headers([], $contextClaims))
                        ->accept('text/event-stream')
                        ->connectTimeout($connectTimeoutSeconds)
                        ->timeout($timeoutSeconds)
                        ->withOptions(['stream' => true])
                        ->get($url);

                    if (! $response->successful()) {
                        $lastException = new \RuntimeException(sprintf(
                            'runtime codex stream returned HTTP %d',
                            $response->status()
                        ));
                    } else {
                        $body = $response->toPsrResponse()->getBody();

                        while (! $body->eof()) {
                            $chunk = $body->read($chunkBytes);

                            if ($chunk === '') {
                                usleep(10_000);

                                continue;
                            }

                            echo $chunk;
                            $this->flushOutput($shouldFlush);
                        }

                        return;
                    }
                } catch (Throwable $exception) {
                    $lastException = $exception;
                }

                if ($attempt < $attempts && $backoffMs > 0) {
                    usleep($backoffMs * 1000);
                }
            }

            Log::error('Runtime codex stream proxy failed', [
                'error' => $lastException?->getMessage(),
                'url' => $url,
            ]);

            $this->emitStreamError(
                $lastException?->getMessage() ?? 'runtime codex stream failed',
                $shouldFlush
            );
        }, 200, $this->streamHeaders());
    }

    /**
     * @param  array<string, mixed>|null  $payload
     * @param  array<string, mixed>  $contextClaims
     * @return array<string, string>
     */
    private function headers(?array $payload, array $contextClaims): array
    {
        $payloadJson = json_encode($payload ?? [], JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
        $payloadJson = $payloadJson === false ? '{}' : $payloadJson;

        $claims = [
            'run_id' => $contextClaims['run_id'] ?? null,
            'thread_id' => $contextClaims['thread_id'] ?? null,
            'user_id' => $contextClaims['user_id'] ?? null,
            'guest_scope' => $contextClaims['guest_scope'] ?? null,
        ];

        $headers = [
            'X-OA-RUNTIME-SIGNATURE' => $this->tokenFactory->issue($claims),
            'X-OA-RUNTIME-KEY-ID' => (string) config('runtime.elixir.signing_key_id', 'runtime-v1'),
            'X-OA-RUNTIME-BODY-SHA256' => hash('sha256', $payloadJson),
            'X-Request-Id' => (string) Str::uuid(),
        ];

        if (is_int($claims['user_id']) && $claims['user_id'] > 0) {
            $headers['X-OA-USER-ID'] = (string) $claims['user_id'];
        }

        if (is_string($claims['guest_scope']) && trim($claims['guest_scope']) !== '') {
            $headers['X-OA-GUEST-SCOPE'] = trim($claims['guest_scope']);
        }

        $lastEventId = $contextClaims['last_event_id'] ?? null;
        if (is_scalar($lastEventId) && trim((string) $lastEventId) !== '') {
            $headers['Last-Event-ID'] = trim((string) $lastEventId);
        }

        $request = request();
        foreach (['traceparent', 'tracestate', 'x-request-id'] as $traceHeader) {
            $value = $request?->header($traceHeader);
            if (is_string($value) && $value !== '') {
                $headers[$traceHeader] = $value;
            }
        }

        return $headers;
    }

    private function normalizeResponseBody(Response $response): mixed
    {
        $json = $response->json();
        if ($json !== null) {
            return $json;
        }

        return $response->body();
    }

    /**
     * @return array<string, string>
     */
    private function streamHeaders(): array
    {
        return [
            'Content-Type' => 'text/event-stream',
            'Cache-Control' => 'no-cache, no-transform',
            'X-Accel-Buffering' => 'no',
        ];
    }

    private function emitStreamError(string $message, bool $shouldFlush): void
    {
        $payload = [
            'error' => [
                'code' => 'runtime_codex_stream_failed',
                'message' => $message,
            ],
        ];

        echo "event: error\n";
        echo 'data: '.json_encode($payload)."\n\n";
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
