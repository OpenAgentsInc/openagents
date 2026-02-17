<?php

namespace App\Exceptions\L402;

use RuntimeException;
use Throwable;

class ApertureReconcileException extends RuntimeException
{
    /**
     * @param  array<string, mixed>  $context
     */
    public function __construct(string $message, private readonly array $context = [], ?Throwable $previous = null)
    {
        parent::__construct($message, 0, $previous);
    }

    /**
     * @return array<string, mixed>
     */
    public function context(): array
    {
        return $this->context;
    }
}
