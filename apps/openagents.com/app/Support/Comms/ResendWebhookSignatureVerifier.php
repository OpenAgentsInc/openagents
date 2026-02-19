<?php

namespace App\Support\Comms;

class ResendWebhookSignatureVerifier
{
    public function verify(string $payload, string $svixId, string $svixTimestamp, string $svixSignature): bool
    {
        $secret = (string) config('runtime.comms.resend.webhook_secret', '');

        if ($secret === '' || $svixId === '' || $svixTimestamp === '' || $svixSignature === '') {
            return false;
        }

        if (! ctype_digit($svixTimestamp)) {
            return false;
        }

        $timestamp = (int) $svixTimestamp;
        $now = now()->unix();
        $tolerance = max(1, (int) config('runtime.comms.resend.webhook_tolerance_seconds', 300));

        if (abs($now - $timestamp) > $tolerance) {
            return false;
        }

        $secretBytes = $this->resolveSecretBytes($secret);
        $signedContent = sprintf('%s.%s.%s', $svixId, $svixTimestamp, $payload);
        $expected = base64_encode(hash_hmac('sha256', $signedContent, $secretBytes, true));

        foreach ($this->extractV1Signatures($svixSignature) as $candidate) {
            if ($candidate !== '' && hash_equals($expected, $candidate)) {
                return true;
            }
        }

        return false;
    }

    /**
     * @return array<int, string>
     */
    private function extractV1Signatures(string $header): array
    {
        $tokens = preg_split('/\s+/', trim($header));
        if (! is_array($tokens)) {
            return [];
        }

        $signatures = [];

        foreach ($tokens as $token) {
            if ($token === '') {
                continue;
            }

            if (str_contains($token, ',')) {
                [$version, $value] = array_pad(explode(',', $token, 2), 2, '');
            } elseif (str_contains($token, '=')) {
                [$version, $value] = array_pad(explode('=', $token, 2), 2, '');
            } else {
                continue;
            }

            if (trim($version) !== 'v1') {
                continue;
            }

            $signatures[] = trim($value);
        }

        return $signatures;
    }

    private function resolveSecretBytes(string $secret): string
    {
        if (str_starts_with($secret, 'whsec_')) {
            $encoded = substr($secret, strlen('whsec_'));
            $decoded = base64_decode($encoded, true);

            if ($decoded !== false && $decoded !== '') {
                return $decoded;
            }

            return $encoded;
        }

        return $secret;
    }
}
