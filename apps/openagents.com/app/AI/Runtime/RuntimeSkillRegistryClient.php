<?php

namespace App\AI\Runtime;

use Illuminate\Http\Client\Response;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Str;
use Throwable;

final class RuntimeSkillRegistryClient
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
                'error' => 'runtime_skill_registry_misconfigured',
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

                $lastError = sprintf('runtime_skill_registry_http_%d', $lastStatus);
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
            'error' => $lastError ?? 'runtime_skill_registry_failed',
        ];
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

        $request = request();
        foreach (['traceparent', 'tracestate', 'x-request-id'] as $traceHeader) {
            $value = $request?->header($traceHeader);
            if (is_string($value) && $value !== '') {
                $headers[$traceHeader] = $value;
            }
        }

        return $headers;
    }

    /**
     * @return mixed
     */
    private function normalizeResponseBody(Response $response): mixed
    {
        $json = $response->json();
        if ($json !== null) {
            return $json;
        }

        return $response->body();
    }
}
