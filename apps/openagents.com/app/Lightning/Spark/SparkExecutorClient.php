<?php

namespace App\Lightning\Spark;

use Illuminate\Http\Client\PendingRequest;
use Illuminate\Support\Facades\Http;

final class SparkExecutorClient
{
    /**
     * @return array<string, mixed>
     */
    public function createWallet(string $walletId, ?string $mnemonic = null): array
    {
        $payload = ['walletId' => $walletId];
        if (is_string($mnemonic) && trim($mnemonic) !== '') {
            $payload['mnemonic'] = trim($mnemonic);
        }

        return $this->request('POST', '/wallets/create', $payload);
    }

    /**
     * @return array<string, mixed>
     */
    public function getWalletStatus(string $walletId, string $mnemonic): array
    {
        return $this->request('POST', '/wallets/status', [
            'walletId' => $walletId,
            'mnemonic' => $mnemonic,
        ]);
    }

    /**
     * @return array<string, mixed>
     */
    public function createInvoice(string $walletId, string $mnemonic, int $amountSats, ?string $description): array
    {
        $payload = [
            'walletId' => $walletId,
            'mnemonic' => $mnemonic,
            'amountSats' => $amountSats,
        ];

        if (is_string($description) && trim($description) !== '') {
            $payload['description'] = trim($description);
        }

        return $this->request('POST', '/wallets/create-invoice', $payload);
    }

    /**
     * @return array<string, mixed>
     */
    public function payBolt11(
        string $walletId,
        string $mnemonic,
        string $invoice,
        int $maxAmountMsats,
        int $timeoutMs,
        ?string $host = null,
    ): array {
        $payload = [
            'walletId' => $walletId,
            'mnemonic' => $mnemonic,
            'invoice' => $invoice,
            'maxAmountMsats' => $maxAmountMsats,
            'timeoutMs' => $timeoutMs,
        ];

        if (is_string($host) && trim($host) !== '') {
            $payload['host'] = strtolower(trim($host));
        }

        return $this->request('POST', '/wallets/pay-bolt11', $payload);
    }

    /**
     * @return array<string, mixed>
     */
    public function sendToSpark(
        string $walletId,
        string $mnemonic,
        string $sparkAddress,
        int $amountSats,
        int $timeoutMs,
    ): array {
        return $this->request('POST', '/wallets/send-spark', [
            'walletId' => $walletId,
            'mnemonic' => $mnemonic,
            'sparkAddress' => $sparkAddress,
            'amountSats' => $amountSats,
            'timeoutMs' => $timeoutMs,
        ]);
    }

    /**
     * @param  array<string, mixed>  $payload
     * @return array<string, mixed>
     */
    private function request(string $method, string $path, array $payload): array
    {
        $baseUrl = trim((string) config('lightning.spark_executor.base_url', ''));
        if ($baseUrl === '') {
            throw new SparkExecutorException('Spark executor not configured. Set SPARK_EXECUTOR_BASE_URL.');
        }

        $timeoutMs = (int) config('lightning.spark_executor.timeout_ms', 20000);
        $request = $this->requestClient($baseUrl, $timeoutMs)->send($method, $path, [
            'json' => $payload,
        ]);

        $body = $request->json();
        if (! is_array($body)) {
            throw new SparkExecutorException('Spark executor returned a non-JSON response.');
        }

        if ($request->failed() || ($body['ok'] ?? true) === false) {
            $error = isset($body['error']) && is_array($body['error']) ? $body['error'] : [];
            $code = is_string($error['code'] ?? null) ? $error['code'] : 'spark_executor_error';
            $message = is_string($error['message'] ?? null)
                ? $error['message']
                : ('Spark executor request failed with HTTP '.$request->status());

            throw SparkExecutorException::fromError($code, $message, [
                'status' => $request->status(),
            ]);
        }

        $result = $body['result'] ?? $body['status'] ?? $body['data'] ?? $body;

        if (! is_array($result)) {
            throw new SparkExecutorException('Spark executor response did not include an object result.');
        }

        return $result;
    }

    private function requestClient(string $baseUrl, int $timeoutMs): PendingRequest
    {
        $timeoutSecs = max(1, (int) ceil($timeoutMs / 1000));

        $client = Http::acceptJson()
            ->contentType('application/json')
            ->baseUrl(rtrim($baseUrl, '/'))
            ->timeout($timeoutSecs)
            ->connectTimeout($timeoutSecs);

        $authToken = trim((string) config('lightning.spark_executor.auth_token', ''));
        if ($authToken !== '') {
            $client = $client->withToken($authToken);
        }

        return $client;
    }
}
