<?php

namespace App\Support\Comms;

use Illuminate\Support\Facades\Http;
use Illuminate\Support\Str;
use Throwable;

class RuntimeCommsDeliveryForwarder
{
    /**
     * @param  array<string, mixed>  $payload
     * @return array{ok: bool, status: int|null, body: mixed, error: string|null}
     */
    public function forward(array $payload): array
    {
        $baseUrl = (string) config('runtime.elixir.base_url', '');
        $path = (string) config('runtime.comms.runtime_delivery_ingest_path', '/internal/v1/comms/delivery-events');
        $signingKey = (string) config('runtime.elixir.signing_key', '');

        if ($baseUrl === '' || $signingKey === '') {
            return [
                'ok' => false,
                'status' => null,
                'body' => null,
                'error' => 'runtime_forward_misconfigured',
            ];
        }

        $maxRetries = max(0, (int) config('runtime.comms.runtime_delivery_max_retries', 2));
        $attempts = $maxRetries + 1;
        $backoffMs = max(0, (int) config('runtime.comms.runtime_delivery_retry_backoff_ms', 200));
        $timeoutMs = max(500, (int) config('runtime.comms.runtime_delivery_timeout_ms', 10000));
        $timeoutSeconds = max(1, (int) ceil($timeoutMs / 1000));

        $url = rtrim($baseUrl, '/').'/'.ltrim($path, '/');

        $lastStatus = null;
        $lastBody = null;
        $lastError = null;

        for ($attempt = 1; $attempt <= $attempts; $attempt++) {
            try {
                $response = Http::withHeaders($this->headers($payload, $signingKey))
                    ->acceptJson()
                    ->connectTimeout($timeoutSeconds)
                    ->timeout($timeoutSeconds)
                    ->post($url, $payload);

                $lastStatus = $response->status();
                $lastBody = $response->json();

                if ($response->successful()) {
                    return [
                        'ok' => true,
                        'status' => $lastStatus,
                        'body' => $lastBody,
                        'error' => null,
                    ];
                }

                $lastError = sprintf('runtime_http_%d', $lastStatus);
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
            'error' => $lastError ?? 'runtime_forward_failed',
        ];
    }

    /**
     * @param  array<string, mixed>  $payload
     * @return array<string, string>
     */
    private function headers(array $payload, string $signingKey): array
    {
        $payloadJson = json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
        $payloadJson = $payloadJson === false ? '{}' : $payloadJson;

        return [
            'X-OA-RUNTIME-SIGNATURE' => $this->signatureToken($signingKey),
            'X-OA-RUNTIME-BODY-SHA256' => hash('sha256', $payloadJson),
            'X-OA-RUNTIME-KEY-ID' => (string) config('runtime.elixir.signing_key_id', 'runtime-v1'),
            'X-Request-Id' => (string) Str::uuid(),
        ];
    }

    private function signatureToken(string $secret): string
    {
        $now = now()->unix();
        $ttl = max(1, (int) config('runtime.elixir.signature_ttl_seconds', 60));

        $claims = [
            'iat' => $now,
            'exp' => $now + $ttl,
            'nonce' => 'nonce-'.(string) Str::uuid(),
        ];

        $payloadSegment = $this->base64UrlEncode(json_encode($claims, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE) ?: '{}');
        $signature = hash_hmac('sha256', $payloadSegment, $secret, true);
        $signatureSegment = $this->base64UrlEncode($signature);

        return sprintf('v1.%s.%s', $payloadSegment, $signatureSegment);
    }

    private function base64UrlEncode(string $value): string
    {
        return rtrim(strtr(base64_encode($value), '+/', '-_'), '=');
    }
}
