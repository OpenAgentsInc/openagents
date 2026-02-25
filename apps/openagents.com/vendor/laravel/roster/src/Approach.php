<?php

namespace Laravel\Roster;

use Laravel\Roster\Enums\Approaches;

class Approach
{
    public function __construct(protected Approaches $approach) {}

    public function name(): string
    {
        return $this->approach->name;
    }

    public function approach(): Approaches
    {
        return $this->approach;
    }
}
