<?php

namespace App\Lightning\L402;

interface InvoicePayer
{
    public function name(): string;

    /**
     * Pay a BOLT11 invoice and return the payment preimage.
     *
     * Implementations must throw on failure.
     */
    public function payBolt11(string $invoice, int $timeoutMs): InvoicePaymentResult;
}
