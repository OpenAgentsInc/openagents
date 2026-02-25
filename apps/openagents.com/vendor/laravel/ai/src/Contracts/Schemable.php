<?php

namespace Laravel\Ai\Contracts;

interface Schemable
{
    /**
     * Get the name of the schema.
     */
    public function name(): string;

    /**
     * Get the array representation of the schema.
     *
     * @return array<string, mixed>
     */
    public function toSchema(): array;
}
