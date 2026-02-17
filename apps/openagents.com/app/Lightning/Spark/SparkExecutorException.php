<?php

namespace App\Lightning\Spark;

use RuntimeException;
use Throwable;

class SparkExecutorException extends RuntimeException
{
    public function __construct(
        string $message,
        private readonly ?string $codeName = null,
        int $code = 0,
        ?Throwable $previous = null,
    ) {
        parent::__construct($message, $code, $previous);
    }

    /**
     * @param  array<string, mixed>|null  $details
     */
    public static function fromError(string $code, string $message, ?array $details = null): self
    {
        $suffix = $details ? ' '.json_encode($details, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE) : '';

        return new self("[{$code}] {$message}{$suffix}", $code);
    }

    public function codeName(): ?string
    {
        return $this->codeName;
    }
}
