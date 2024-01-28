<?php

namespace App\Services\Alby\Contracts;

interface AlbyClient
{
    public function isConnectionValid(): bool;

    public function getInfo(): array;

    public function addInvoice($invoice): array;

    public function getInvoice($checkingId): array;

    public function isInvoicePaid($checkingId): bool;
}
