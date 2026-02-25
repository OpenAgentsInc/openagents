<?php

namespace Laravel\Roster\Scanners;

use Illuminate\Support\Collection;
use Laravel\Roster\Enums\NodePackageManager;

class PackageLock
{
    /**
     * @param  string  $path  - Base path to scan for lock files (package-lock.json, pnpm-lock.yaml, yarn.lock, ...)
     */
    public function __construct(protected string $path) {}

    /**
     * @return \Illuminate\Support\Collection<int, \Laravel\Roster\Package|\Laravel\Roster\Approach>
     */
    public function scan(): Collection
    {
        foreach (NodePackageManager::cases() as $case) {
            $scanner = $case->scanner($this->path);
            if ($scanner->canScan()) {
                return $scanner->scan();
            }
        }

        return collect();
    }

    public function detect(): NodePackageManager
    {
        foreach (NodePackageManager::cases() as $case) {
            $scanner = $case->scanner($this->path);
            if ($scanner->canScan()) {
                return $case;
            }
        }

        return NodePackageManager::NPM;
    }
}
