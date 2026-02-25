<?php

declare(strict_types=1);

namespace Laravel\Boost\Install\Enums;

enum Platform: string
{
    case Darwin = 'darwin';
    case Linux = 'linux';
    case Windows = 'windows';

    public static function current(): self
    {
        return match (PHP_OS_FAMILY) {
            'Windows' => self::Windows,
            'Darwin' => self::Darwin,
            default => self::Linux,
        };
    }
}
