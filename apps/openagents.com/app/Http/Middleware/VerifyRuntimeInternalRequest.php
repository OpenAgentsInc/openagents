<?php

namespace App\Http\Middleware;

use Closure;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Cache;

class VerifyRuntimeInternalRequest
{
    /**
     * Handle an incoming request.
     */
    public function handle(Request $request, Closure $next): mixed
    {
        $secret = (string) config('runtime.internal.shared_secret', '');

        if ($secret === '') {
            return $this->reject('runtime internal auth misconfigured', 500, 'internal_auth_misconfigured');
        }

        $keyId = (string) $request->header('x-oa-internal-key-id', '');
        $expectedKeyId = (string) config('runtime.internal.key_id', 'runtime-internal-v1');

        if ($keyId === '' || ! hash_equals($expectedKeyId, $keyId)) {
            return $this->reject('invalid key id', 401, 'invalid_key_id');
        }

        $timestamp = (string) $request->header('x-oa-internal-timestamp', '');
        $nonce = (string) $request->header('x-oa-internal-nonce', '');
        $providedBodyHash = (string) $request->header('x-oa-internal-body-sha256', '');
        $providedSignature = (string) $request->header('x-oa-internal-signature', '');

        if ($timestamp === '' || $nonce === '' || $providedBodyHash === '' || $providedSignature === '') {
            return $this->reject('missing auth headers', 401, 'missing_auth_headers');
        }

        if (! ctype_digit($timestamp)) {
            return $this->reject('invalid timestamp', 401, 'invalid_timestamp');
        }

        $ttlSeconds = max(1, (int) config('runtime.internal.signature_ttl_seconds', 60));
        $timestampInt = (int) $timestamp;
        $now = now()->unix();

        if (abs($now - $timestampInt) > $ttlSeconds) {
            return $this->reject('signature expired', 401, 'signature_expired');
        }

        $body = (string) $request->getContent();
        $computedBodyHash = hash('sha256', $body);

        if (! hash_equals($computedBodyHash, $providedBodyHash)) {
            return $this->reject('body hash mismatch', 401, 'body_hash_mismatch');
        }

        $expectedSignature = hash_hmac('sha256', implode("\n", [$timestamp, $nonce, $computedBodyHash]), $secret);

        if (! hash_equals($expectedSignature, $providedSignature)) {
            return $this->reject('invalid signature', 401, 'invalid_signature');
        }

        $nonceKey = sprintf('runtime_internal_nonce:%s:%s', $keyId, $nonce);
        $nonceAccepted = Cache::add($nonceKey, true, now()->addSeconds($ttlSeconds + 5));

        if (! $nonceAccepted) {
            return $this->reject('nonce replay detected', 401, 'nonce_replay');
        }

        return $next($request);
    }

    private function reject(string $message, int $status, string $code): JsonResponse
    {
        return response()->json([
            'error' => [
                'code' => $code,
                'message' => $message,
            ],
        ], $status);
    }
}
