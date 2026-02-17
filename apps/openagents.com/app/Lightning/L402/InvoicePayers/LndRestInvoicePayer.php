<?php

namespace App\Lightning\L402\InvoicePayers;

use App\Lightning\L402\InvoicePayer;
use App\Lightning\L402\InvoicePaymentResult;
use Illuminate\Support\Facades\Http;
use RuntimeException;

final class LndRestInvoicePayer implements InvoicePayer
{
    public function name(): string
    {
        return 'lnd_rest';
    }

    /**
     * @param  array<string, mixed>  $context
     */
    public function payBolt11(string $invoice, int $timeoutMs, array $context = []): InvoicePaymentResult
    {
        $baseUrl = (string) config('lightning.lnd_rest.base_url');
        $macaroonHex = (string) config('lightning.lnd_rest.macaroon_hex');

        if ($baseUrl === '' || $macaroonHex === '') {
            throw new RuntimeException('LND REST payer not configured. Set LND_REST_BASE_URL and LND_REST_MACAROON_HEX.');
        }

        $verifyOpt = $this->resolveVerifyOption();

        $timeoutSeconds = max(1, (int) ceil($timeoutMs / 1000));

        $resp = Http::withOptions(['verify' => $verifyOpt])
            ->timeout($timeoutSeconds)
            ->withHeaders([
                'Grpc-Metadata-macaroon' => $macaroonHex,
            ])
            ->post(rtrim($baseUrl, '/').'/v1/channels/transactions', [
                'payment_request' => $invoice,
            ]);

        if (! $resp->successful()) {
            throw new RuntimeException('LND REST pay failed: HTTP '.$resp->status().' '.$resp->body());
        }

        $json = $resp->json();
        if (! is_array($json)) {
            throw new RuntimeException('LND REST pay failed: invalid JSON response');
        }

        $paymentError = $json['payment_error'] ?? '';
        if (is_string($paymentError) && trim($paymentError) !== '') {
            throw new RuntimeException('LND REST pay failed: '.$paymentError);
        }

        $preimage = $json['payment_preimage'] ?? null;
        if (! is_string($preimage) || $preimage === '') {
            throw new RuntimeException('LND REST pay failed: missing payment_preimage');
        }

        $preimageHex = $this->normalizePreimageToHex($preimage);

        return new InvoicePaymentResult(
            preimage: $preimageHex,
            paymentId: isset($json['payment_hash']) && is_string($json['payment_hash']) ? $json['payment_hash'] : null,
        );
    }

    private function normalizePreimageToHex(string $value): string
    {
        $value = trim($value);

        if (preg_match('/^[0-9a-f]{64}$/i', $value) === 1) {
            return strtolower($value);
        }

        $decoded = base64_decode($value, true);
        if ($decoded === false) {
            throw new RuntimeException('LND REST pay failed: payment_preimage is neither hex nor base64');
        }

        return bin2hex($decoded);
    }

    private function resolveVerifyOption(): bool|string
    {
        $certBase64 = config('lightning.lnd_rest.tls_cert_base64');
        if (is_string($certBase64) && trim($certBase64) !== '') {
            $certPath = storage_path('app/lnd/tls.cert');
            if (! is_dir(dirname($certPath))) {
                mkdir(dirname($certPath), 0700, true);
            }

            $decoded = base64_decode(trim($certBase64), true);
            if ($decoded === false) {
                throw new RuntimeException('Invalid base64 in LND_REST_TLS_CERT_BASE64');
            }

            // Write only if changed.
            if (! file_exists($certPath) || hash_file('sha256', $certPath) !== hash('sha256', $decoded)) {
                file_put_contents($certPath, $decoded);
            }

            return $certPath;
        }

        $verify = config('lightning.lnd_rest.tls_verify', true);

        if (is_bool($verify)) {
            return $verify;
        }

        if (is_string($verify)) {
            $maybeBool = filter_var($verify, FILTER_VALIDATE_BOOLEAN, FILTER_NULL_ON_FAILURE);
            if (is_bool($maybeBool)) {
                return $maybeBool;
            }
        }

        return true;
    }
}
