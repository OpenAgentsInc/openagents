<?php

namespace Laravel\Ai\Files;

use Laravel\Ai\Contracts\Files\HasName;

abstract class File implements HasName
{
    public ?string $name = null;

    /**
     * Get the displayable name of the file.
     */
    public function name(): ?string
    {
        return $this->name;
    }

    /**
     * Set the displayable name of the file.
     */
    public function as(?string $name): static
    {
        $this->name = $name;

        return $this;
    }
}
