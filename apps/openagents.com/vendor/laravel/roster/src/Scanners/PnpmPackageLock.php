<?php

namespace Laravel\Roster\Scanners;

use Illuminate\Support\Collection;
use Illuminate\Support\Facades\Log;
use Symfony\Component\Yaml\Yaml;

class PnpmPackageLock extends BasePackageScanner
{
    protected function lockFile(): string
    {
        return 'pnpm-lock.yaml';
    }

    /**
     * @return \Illuminate\Support\Collection<int, \Laravel\Roster\Package|\Laravel\Roster\Approach>
     */
    public function scan(): Collection
    {
        $mappedItems = collect();
        $lockFilePath = $this->lockFilePath();

        $contents = $this->validateFile($lockFilePath, 'PNPM lock');
        if ($contents === null) {
            return $mappedItems;
        }

        try {
            /** @var array<string, mixed> $parsed */
            $parsed = Yaml::parse($contents);
        } catch (\Exception $e) {
            Log::error('Failed to parse YAML: '.$e->getMessage());

            return $mappedItems;
        }

        /** @var array<string, string> $dependencies */
        $dependencies = [];
        /** @var array<string, string> $devDependencies */
        $devDependencies = [];

        /** @var array<string, array<string, mixed>> $importers */
        $importers = $parsed['importers'] ?? [];
        $root = $importers['.'] ?? [];
        /** @var array<string, array<string, mixed>> $rootDependencies */
        $rootDependencies = $root['dependencies'] ?? [];
        /** @var array<string, array<string, mixed>> $rootDevDependencies */
        $rootDevDependencies = $root['devDependencies'] ?? [];

        foreach ($rootDependencies as $name => $data) {
            if (isset($data['version'])) {
                $dependencies[$name] = $data['version'];
            }
        }

        foreach ($rootDevDependencies as $name => $data) {
            if (isset($data['version'])) {
                $devDependencies[$name] = $data['version'];
            }
        }

        /** @var array<string, string> $dependencies */
        /** @var array<string, string> $devDependencies */
        $this->processDependencies($dependencies, $mappedItems, false);
        $this->processDependencies($devDependencies, $mappedItems, true);

        return $mappedItems;
    }
}
