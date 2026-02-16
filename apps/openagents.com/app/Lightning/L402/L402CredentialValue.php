<?php

namespace App\Lightning\L402;

use Carbon\CarbonImmutable;

final class L402CredentialValue
{
    public function __construct(
        public readonly string $macaroon,
        public readonly string $preimage,
        public readonly CarbonImmutable $expiresAt,
    ) {}
}
