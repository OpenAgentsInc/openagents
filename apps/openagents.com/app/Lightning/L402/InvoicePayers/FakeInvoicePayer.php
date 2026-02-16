<?php

namespace App\Lightning\L402\InvoicePayers;

use App\Lightning\L402\InvoicePayer;
use App\Lightning\L402\InvoicePaymentResult;

final class FakeInvoicePayer implements InvoicePayer
{
    public function name(): string
    {
        return 'fake';
    }

    public function payBolt11(string $invoice, int $timeoutMs): InvoicePaymentResult
    {
        // Deterministic "preimage" for tests and local demos.
        $preimage = hash('sha256', 'preimage:'.$invoice);

        return new InvoicePaymentResult(
            preimage: $preimage,
            paymentId: 'fake:'.substr(hash('sha256', 'payment:'.$invoice), 0, 16),
        );
    }
}
