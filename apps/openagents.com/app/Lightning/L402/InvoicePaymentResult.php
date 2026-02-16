<?php

namespace App\Lightning\L402;

final class InvoicePaymentResult
{
    public function __construct(
        public readonly string $preimage,
        public readonly ?string $paymentId = null,
    ) {}
}
