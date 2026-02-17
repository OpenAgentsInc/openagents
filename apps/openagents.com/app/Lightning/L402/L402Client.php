<?php

namespace App\Lightning\L402;

use Illuminate\Support\Facades\Http;
use RuntimeException;

final class L402Client
{
    public function __construct(
        private readonly WwwAuthenticateParser $wwwAuthenticateParser,
        private readonly L402CredentialCache $credentialCache,
        private readonly InvoicePayer $invoicePayer,
    ) {}

    /**
     * Perform an L402 request with strict cap + allowlist enforcement.
     *
     * @param  array<string, string>  $headers
     */
    public function fetch(string $url, string $method, array $headers, ?string $body, int $maxSpendSats, string $scope, array $context = []): array
    {
        $host = $this->hostFromUrl($url);
        $method = strtoupper(trim($method));

        $maxSpendMsats = $this->safeMsatsFromSats($maxSpendSats);

        if (! $this->isHostAllowed($host, $context)) {
            return [
                'status' => 'blocked',
                'paid' => false,
                'cacheHit' => false,
                'paymentBackend' => $this->invoicePayer->name(),
                'denyCode' => 'domain_not_allowed',
                'denyReason' => [
                    'host' => $host,
                    'allowlistHosts' => $this->resolveAllowlist($context),
                ],
                'host' => $host,
                'url' => $url,
                'method' => $method,
                'scope' => $scope,
                'maxSpendSats' => $maxSpendSats,
                'maxSpendMsats' => $maxSpendMsats,
            ];
        }

        $headers = $this->normalizeHeaders($headers);

        // 1) Try cached credential first.
        $cached = $this->credentialCache->get($host, $scope);
        if ($cached) {
            $resp = $this->sendHttpRequest(
                url: $url,
                method: $method,
                headers: $this->withAuthorization($headers, $cached->macaroon, $cached->preimage),
                body: $body,
            );

            if ($resp->successful()) {
                return array_merge([
                    'status' => 'cached',
                    'paid' => false,
                    'cacheHit' => true,
                    'cacheStatus' => 'hit',
                    'paymentBackend' => $this->invoicePayer->name(),
                    'proofReference' => $this->proofReference($cached->preimage),
                    'host' => $host,
                    'url' => $url,
                    'method' => $method,
                    'scope' => $scope,
                    'maxSpendSats' => $maxSpendSats,
                    'maxSpendMsats' => $maxSpendMsats,
                ], $this->captureResponse($resp));
            }

            // If the cache is rejected, invalidate and fall back to the full flow.
            if (in_array($resp->status(), [401, 402, 403], true)) {
                $this->credentialCache->delete($host, $scope);
            }
        }

        // 2) Request without authorization.
        $initial = $this->sendHttpRequest(
            url: $url,
            method: $method,
            headers: $headers,
            body: $body,
        );

        if ($initial->status() !== 402) {
            // Not an L402 gate (or already free). Return best-effort capture.
            return array_merge([
                'status' => 'completed',
                'paid' => false,
                'cacheHit' => false,
                'cacheStatus' => 'none',
                'paymentBackend' => $this->invoicePayer->name(),
                'host' => $host,
                'url' => $url,
                'method' => $method,
                'scope' => $scope,
                'maxSpendSats' => $maxSpendSats,
                'maxSpendMsats' => $maxSpendMsats,
                'responseStatusCode' => $initial->status(),
            ], $this->captureResponse($initial));
        }

        $challenge = $this->wwwAuthenticateParser->parseL402Challenge($initial->header('www-authenticate'));
        if (! $challenge) {
            return [
                'status' => 'failed',
                'paid' => false,
                'cacheHit' => false,
                'paymentBackend' => $this->invoicePayer->name(),
                'host' => $host,
                'url' => $url,
                'method' => $method,
                'scope' => $scope,
                'maxSpendSats' => $maxSpendSats,
                'maxSpendMsats' => $maxSpendMsats,
                'denyCode' => 'missing_l402_challenge',
                'denyReason' => [
                    'status' => 402,
                ],
            ];
        }

        $quotedAmountMsats = Bolt11::amountMsats($challenge->invoice);
        if (! is_int($quotedAmountMsats)) {
            return [
                'status' => 'blocked',
                'paid' => false,
                'cacheHit' => false,
                'paymentBackend' => $this->invoicePayer->name(),
                'denyCode' => 'quoted_amount_missing',
                'denyReason' => [
                    'invoice_prefix' => substr($challenge->invoice, 0, 16),
                ],
                'host' => $host,
                'url' => $url,
                'method' => $method,
                'scope' => $scope,
                'maxSpendSats' => $maxSpendSats,
                'maxSpendMsats' => $maxSpendMsats,
            ];
        }

        if ($quotedAmountMsats > $maxSpendMsats) {
            return [
                'status' => 'blocked',
                'paid' => false,
                'cacheHit' => false,
                'paymentBackend' => $this->invoicePayer->name(),
                'denyCode' => 'quoted_cost_exceeds_cap',
                'denyReason' => [
                    'quotedAmountMsats' => $quotedAmountMsats,
                    'maxSpendMsats' => $maxSpendMsats,
                ],
                'host' => $host,
                'url' => $url,
                'method' => $method,
                'scope' => $scope,
                'maxSpendSats' => $maxSpendSats,
                'maxSpendMsats' => $maxSpendMsats,
                'quotedAmountMsats' => $quotedAmountMsats,
            ];
        }

        $timeoutMs = (int) config('lightning.l402.payment_timeout_ms', 12000);

        // 3) Pay invoice -> preimage.
        $paymentContext = array_merge($context, [
            'host' => $host,
            'scope' => $scope,
            'url' => $url,
            'method' => $method,
            'maxSpendSats' => $maxSpendSats,
            'maxSpendMsats' => $maxSpendMsats,
            'quotedAmountMsats' => $quotedAmountMsats,
        ]);

        $payment = $this->invoicePayer->payBolt11($challenge->invoice, $timeoutMs, $paymentContext);
        $preimage = $payment->preimage;

        if (! is_string($preimage) || preg_match('/^[0-9a-f]{64}$/i', $preimage) !== 1) {
            throw new RuntimeException('Invoice payer returned an invalid preimage.');
        }

        // 4) Retry with Authorization header.
        $paidResp = $this->sendHttpRequest(
            url: $url,
            method: $method,
            headers: $this->withAuthorization($headers, $challenge->macaroon, $preimage),
            body: $body,
        );

        if (! $paidResp->successful()) {
            return [
                'status' => 'failed',
                'paid' => true,
                'cacheHit' => false,
                'paymentBackend' => $this->invoicePayer->name(),
                'proofReference' => $this->proofReference($preimage),
                'host' => $host,
                'url' => $url,
                'method' => $method,
                'scope' => $scope,
                'maxSpendSats' => $maxSpendSats,
                'maxSpendMsats' => $maxSpendMsats,
                'quotedAmountMsats' => $quotedAmountMsats,
                'amountMsats' => $quotedAmountMsats,
                'responseStatusCode' => $paidResp->status(),
            ];
        }

        // 5) Cache credential.
        $ttlSeconds = (int) config('lightning.l402.credential_ttl_seconds', 600);
        $this->credentialCache->put($host, $scope, $challenge->macaroon, $preimage, $ttlSeconds);

        return array_merge([
            'status' => 'completed',
            'paid' => true,
            'cacheHit' => false,
            'cacheStatus' => 'miss',
            'paymentBackend' => $this->invoicePayer->name(),
            'proofReference' => $this->proofReference($preimage),
            'host' => $host,
            'url' => $url,
            'method' => $method,
            'scope' => $scope,
            'maxSpendSats' => $maxSpendSats,
            'maxSpendMsats' => $maxSpendMsats,
            'quotedAmountMsats' => $quotedAmountMsats,
            'amountMsats' => $quotedAmountMsats,
        ], $this->captureResponse($paidResp));
    }

    private function sendHttpRequest(string $url, string $method, array $headers, ?string $body)
    {
        $req = Http::withHeaders($headers);

        if (is_string($body)) {
            $contentType = $this->headerValue($headers, 'content-type') ?? 'application/octet-stream';

            $req = $req->withBody($body, $contentType);
        }

        return $req->send($method, $url);
    }

    /**
     * @param  array<string, string>  $headers
     * @return array<string, string>
     */
    private function normalizeHeaders(array $headers): array
    {
        $out = [];
        foreach ($headers as $k => $v) {
            if (! is_string($k) || ! is_string($v)) {
                continue;
            }

            if (strtolower($k) === 'authorization') {
                continue;
            }

            $out[$k] = $v;
        }

        return $out;
    }

    /**
     * @param  array<string, string>  $headers
     * @return array<string, string>
     */
    private function withAuthorization(array $headers, string $macaroon, string $preimage): array
    {
        $headers['Authorization'] = 'L402 '.$macaroon.':'.$preimage;

        return $headers;
    }

    private function proofReference(string $preimage): string
    {
        return 'preimage:'.substr($preimage, 0, 16);
    }

    private function hostFromUrl(string $url): string
    {
        $host = parse_url($url, PHP_URL_HOST);
        if (! is_string($host) || $host === '') {
            throw new RuntimeException('Invalid url: missing host');
        }

        return strtolower($host);
    }

    private function isHostAllowed(string $host, array $context): bool
    {
        if (! $this->shouldEnforceAllowlist($context)) {
            return true;
        }

        $allow = $this->resolveAllowlist($context);

        if ($allow === []) {
            return false;
        }

        return in_array(strtolower($host), $allow, true);
    }

    private function shouldEnforceAllowlist(array $context): bool
    {
        if (array_key_exists('allowedHosts', $context)) {
            return true;
        }

        return (bool) config('lightning.l402.enforce_host_allowlist', false);
    }

    /**
     * @return array<int, string>
     */
    private function resolveAllowlist(array $context): array
    {
        $rawAllowlist = $context['allowedHosts'] ?? config('lightning.l402.allowlist_hosts', []);

        if (! is_array($rawAllowlist) || $rawAllowlist === []) {
            return [];
        }

        $allow = [];

        foreach ($rawAllowlist as $host) {
            if (! is_string($host)) {
                continue;
            }

            $candidate = strtolower(trim($host));
            if ($candidate === '') {
                continue;
            }

            $allow[$candidate] = $candidate;
        }

        return array_values($allow);
    }

    private function safeMsatsFromSats(int $sats): int
    {
        $sats = max(0, $sats);

        if ($sats > intdiv(PHP_INT_MAX, 1000)) {
            return PHP_INT_MAX;
        }

        return $sats * 1000;
    }

    /**
     * @return array<string, mixed>
     */
    private function captureResponse($resp): array
    {
        $body = (string) $resp->body();
        $bytes = strlen($body);

        $maxBytes = (int) config('lightning.l402.response_max_bytes', 65536);
        $previewBytes = (int) config('lightning.l402.response_preview_bytes', 1024);

        $captured = $body;
        $truncated = false;

        if ($bytes > $maxBytes) {
            $captured = substr($body, 0, $maxBytes);
            $truncated = true;
        }

        $preview = $captured;
        if (strlen($preview) > $previewBytes) {
            $preview = substr($captured, 0, $previewBytes);
        }

        return [
            'responseStatusCode' => $resp->status(),
            'responseContentType' => $resp->header('content-type'),
            'responseBytes' => strlen($captured),
            'responseTruncated' => $truncated,
            'responseBodyTextPreview' => $preview,
            'responseBodySha256' => hash('sha256', $captured),
        ];
    }

    /**
     * @param  array<string, string>  $headers
     */
    private function headerValue(array $headers, string $key): ?string
    {
        $k = strtolower($key);

        foreach ($headers as $hk => $hv) {
            if (! is_string($hk) || ! is_string($hv)) {
                continue;
            }

            if (strtolower($hk) === $k) {
                return $hv;
            }
        }

        return null;
    }
}
