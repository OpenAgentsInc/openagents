<?php

declare(strict_types=1);

namespace Pest\Profanity\Contracts;

/**
 * @internal
 */
interface Logger
{
    public function append(string $path, array $profanity): void;

    /**
     * Outputs the coverage report.
     */
    public function output(): void;
}
