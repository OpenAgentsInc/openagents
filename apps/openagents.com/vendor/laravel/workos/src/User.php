<?php

namespace Laravel\WorkOS;

class User
{
    public function __construct(
        public string $id,
        public ?string $organizationId,
        public ?string $firstName,
        public ?string $lastName,
        public string $email,
        public ?string $avatar = null,
    ) {}
}
