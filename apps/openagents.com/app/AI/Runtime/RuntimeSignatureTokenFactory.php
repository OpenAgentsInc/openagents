<?php

namespace App\AI\Runtime;

use Illuminate\Support\Str;

final class RuntimeSignatureTokenFactory
{
    /**
     * @param  array<string, mixed>  $claims
     */
    public function issue(array $claims = []): string
    {
        $secret = (string) config('runtime.elixir.signing_key', '');
        $ttlSeconds = max(1, (int) config('runtime.elixir.signature_ttl_seconds', 60));
        $now = now()->unix();

        $payload = [
            'iat' => $now,
            'exp' => $now + $ttlSeconds,
            'nonce' => 'nonce-'.(string) Str::uuid(),
        ];

        foreach (['run_id', 'thread_id', 'guest_scope', 'user_id'] as $key) {
            if (array_key_exists($key, $claims) && $claims[$key] !== null && $claims[$key] !== '') {
                $payload[$key] = $claims[$key];
            }
        }

        $payloadJson = json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
        $payloadJson = $payloadJson === false ? '{}' : $payloadJson;

        $payloadSegment = $this->base64UrlEncode($payloadJson);
        $signature = hash_hmac('sha256', $payloadSegment, $secret, true);
        $signatureSegment = $this->base64UrlEncode($signature);

        return sprintf('v1.%s.%s', $payloadSegment, $signatureSegment);
    }

    private function base64UrlEncode(string $value): string
    {
        return rtrim(strtr(base64_encode($value), '+/', '-_'), '=');
    }
}

