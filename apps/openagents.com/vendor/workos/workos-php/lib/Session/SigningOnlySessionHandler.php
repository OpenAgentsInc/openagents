<?php

namespace WorkOS\Session;

use WorkOS\Exception\UnexpectedValueException;

/**
 * Session handler using HMAC signing only (no encryption).
 *
 * WARNING: This handler does NOT encrypt session data. The session contents
 * are readable by anyone with access to the cookie. Only use this in
 * controlled environments with TLS where you trust the transport layer.
 *
 * Use cases:
 * - Performance-critical applications where encryption overhead matters
 * - Environments with strict TLS enforcement
 * - Debugging/development scenarios
 */
class SigningOnlySessionHandler implements SessionEncryptionInterface
{
    const ALGORITHM = 'sha256';
    const VERSION = 1;
    const DEFAULT_TTL = 2592000; // 30 days

    /**
     * Seal session data with HMAC signature (no encryption).
     *
     * Format: base64(json({ p: base64(payload), s: base64(signature) }))
     * Payload: json({ v: version, d: data, e: expiry })
     *
     * @param array $data Session data
     * @param string $password HMAC key
     * @param int|null $ttl Time-to-live in seconds
     * @return string Signed session string
     */
    public function seal(array $data, string $password, ?int $ttl = null): string
    {
        $ttl = $ttl ?? self::DEFAULT_TTL;
        $expiry = time() + $ttl;

        $payload = [
            'v' => self::VERSION,
            'd' => $data,
            'e' => $expiry,
        ];

        $payloadJson = json_encode($payload);
        $signature = hash_hmac(self::ALGORITHM, $payloadJson, $password, true);

        $sealed = [
            'p' => base64_encode($payloadJson),
            's' => base64_encode($signature),
        ];

        return base64_encode(json_encode($sealed));
    }

    /**
     * Unseal session data by verifying HMAC signature.
     *
     * @param string $sealed Signed session string
     * @param string $password HMAC key
     * @return array Unsealed session data
     * @throws UnexpectedValueException If signature invalid or expired
     */
    public function unseal(string $sealed, string $password): array
    {
        $decoded = json_decode(base64_decode($sealed), true);
        if (!$decoded || !isset($decoded['p']) || !isset($decoded['s'])) {
            throw new UnexpectedValueException('Invalid signed session format');
        }

        $payloadJson = base64_decode($decoded['p']);
        $providedSignature = base64_decode($decoded['s']);
        $expectedSignature = hash_hmac(self::ALGORITHM, $payloadJson, $password, true);

        // Constant-time comparison to prevent timing attacks
        if (!hash_equals($expectedSignature, $providedSignature)) {
            throw new UnexpectedValueException('Invalid session signature');
        }

        $payload = json_decode($payloadJson, true);
        if (!$payload || !isset($payload['v']) || !isset($payload['d']) || !isset($payload['e'])) {
            throw new UnexpectedValueException('Invalid payload structure');
        }

        // Version check for future compatibility
        if ($payload['v'] !== self::VERSION) {
            throw new UnexpectedValueException('Unsupported session version');
        }

        // TTL check
        if ($payload['e'] < time()) {
            throw new UnexpectedValueException('Session expired');
        }

        return $payload['d'];
    }
}
