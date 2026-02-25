<?php

namespace Laravel\Roster\Enums;

use Laravel\Roster\Scanners\BunPackageLock;
use Laravel\Roster\Scanners\NpmPackageLock;
use Laravel\Roster\Scanners\PnpmPackageLock;
use Laravel\Roster\Scanners\YarnPackageLock;

enum NodePackageManager: string
{
    case NPM = 'npm';
    case PNPM = 'pnpm';
    case YARN = 'yarn';
    case BUN = 'bun';

    public function scanner(string $path): NpmPackageLock|PnpmPackageLock|YarnPackageLock|BunPackageLock
    {
        return match ($this) {
            self::NPM => new NpmPackageLock($path),
            self::PNPM => new PnpmPackageLock($path),
            self::YARN => new YarnPackageLock($path),
            self::BUN => new BunPackageLock($path),
        };
    }
}
