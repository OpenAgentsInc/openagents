<?php

declare(strict_types=1);

namespace Pest\Profanity;

/**
 * @internal
 */
final class Error
{
    /**
     * Creates a new profanity error instance.
     */
    public function __construct(
        public readonly string $file,
        public readonly int $line,
        public readonly string $word,
    ) {
        //
    }

    /**
     * Returns the short type of the error.
     */
    public function getShortType(): string
    {
        return 'pr';
    }
}
