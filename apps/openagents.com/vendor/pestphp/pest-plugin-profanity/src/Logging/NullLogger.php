<?php

declare(strict_types=1);

namespace Pest\Profanity\Logging;

use Pest\Profanity\Contracts\Logger;

/**
 * @internal
 */
final class NullLogger implements Logger
{
    /**
     * {@inheritDoc}
     */
    public function append(string $path, array $profanity): void
    {
        //
    }

    /**
     * {@inheritDoc}
     */
    public function output(): void
    {
        //
    }
}
