<?php

declare(strict_types=1);

namespace Pest\Profanity;

/**
 * @internal
 */
final class Result
{
    /**
     * Creates a new result instance.
     *
     * @param  array<int, Error>  $errors
     */
    public function __construct(
        public readonly string $file,
        public readonly array $errors,
    ) {
        //
    }
}
