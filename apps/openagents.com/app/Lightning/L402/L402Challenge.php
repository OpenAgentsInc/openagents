<?php

namespace App\Lightning\L402;

final class L402Challenge
{
    public function __construct(
        public readonly string $macaroon,
        public readonly string $invoice,
    ) {}
}
