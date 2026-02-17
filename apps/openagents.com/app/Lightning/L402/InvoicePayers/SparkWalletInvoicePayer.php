<?php

namespace App\Lightning\L402\InvoicePayers;

use App\Lightning\L402\Bolt11;
use App\Lightning\L402\InvoicePayer;
use App\Lightning\L402\InvoicePaymentResult;
use App\Lightning\Spark\UserSparkWalletService;
use RuntimeException;
use Throwable;

final class SparkWalletInvoicePayer implements InvoicePayer
{
    public function __construct(private readonly UserSparkWalletService $wallets) {}

    public function name(): string
    {
        return 'spark_wallet';
    }

    /**
     * @param  array<string, mixed>  $context
     */
    public function payBolt11(string $invoice, int $timeoutMs, array $context = []): InvoicePaymentResult
    {
        $userId = $this->resolveUserId($context);
        if (! is_int($userId) || $userId <= 0) {
            throw new RuntimeException('Spark wallet payer requires authenticated user context.');
        }

        $maxAmountMsats = $this->resolveMaxAmountMsats($invoice, $context);
        $host = is_string($context['host'] ?? null) ? strtolower(trim((string) $context['host'])) : null;

        try {
            $result = $this->wallets->payBolt11(
                user: $userId,
                invoice: $invoice,
                maxAmountMsats: $maxAmountMsats,
                timeoutMs: $timeoutMs,
                host: $host,
            );
        } catch (Throwable $e) {
            throw new RuntimeException('Spark wallet payment failed: '.$e->getMessage(), previous: $e);
        }

        $preimage = $this->firstString($result, [
            'preimage',
            'paymentPreimage',
            'payment.preimage',
            'payment.paymentPreimage',
        ]);

        if (! is_string($preimage) || $preimage === '') {
            throw new RuntimeException('Spark wallet payer did not return a payment preimage.');
        }

        $paymentId = $this->firstString($result, [
            'paymentId',
            'paymentHash',
            'payment.paymentId',
            'payment.paymentHash',
        ]);

        return new InvoicePaymentResult(
            preimage: strtolower(trim($preimage)),
            paymentId: is_string($paymentId) && $paymentId !== '' ? $paymentId : null,
        );
    }

    /**
     * @param  array<string, mixed>  $context
     */
    private function resolveUserId(array $context): ?int
    {
        $fromContext = $context['userId'] ?? null;
        if (is_int($fromContext)) {
            return $fromContext;
        }
        if (is_numeric($fromContext)) {
            return (int) $fromContext;
        }

        $authId = auth()->id();

        if (is_int($authId)) {
            return $authId;
        }

        if (is_numeric($authId)) {
            return (int) $authId;
        }

        return null;
    }

    /**
     * @param  array<string, mixed>  $context
     */
    private function resolveMaxAmountMsats(string $invoice, array $context): int
    {
        $fromContext = $context['maxSpendMsats'] ?? null;
        if (is_int($fromContext) && $fromContext > 0) {
            return $fromContext;
        }

        if (is_numeric($fromContext)) {
            $value = (int) $fromContext;
            if ($value > 0) {
                return $value;
            }
        }

        $fromInvoice = Bolt11::amountMsats($invoice);
        if (is_int($fromInvoice) && $fromInvoice > 0) {
            return $fromInvoice;
        }

        throw new RuntimeException('Spark wallet payer could not resolve maxAmountMsats.');
    }

    /**
     * @param  array<string, mixed>  $result
     * @param  list<string>  $paths
     */
    private function firstString(array $result, array $paths): ?string
    {
        foreach ($paths as $path) {
            if (str_contains($path, '.')) {
                $value = data_get($result, $path);
            } else {
                $value = $result[$path] ?? null;
            }

            if (is_string($value) && trim($value) !== '') {
                return trim($value);
            }
        }

        return null;
    }
}
