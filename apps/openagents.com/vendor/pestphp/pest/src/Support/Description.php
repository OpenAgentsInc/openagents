<?php

declare(strict_types=1);

namespace Pest\Support;

final readonly class Description implements \Stringable
{
    /**
     * Creates a new Description instance.
     */
    public function __construct(private string $description) {}

    /**
     * Returns the description as a string.
     */
    public function __toString(): string
    {
        return $this->description;
    }
}
