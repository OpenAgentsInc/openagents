<?php

namespace Laravel\Roster\Scanners;

use Illuminate\Support\Collection;
use Laravel\Roster\Approach;
use Laravel\Roster\Package;

class YarnPackageLock extends BasePackageScanner
{
    private const YARN_V1_HEADER = '/^("?)(@[^@"\/]+\/[^@"]+|[^@"]+)(@[^:"]+)?\1:$/';

    private const YARN_V4_HEADER = '/^"(@?[^@"]+(?:\/[^@"]+)?)@npm:[^"]*":\s*$/';

    private const YARN_V1_VERSION = '/^version\s+"([^"]+)"$/';

    private const YARN_V4_VERSION = '/^version:\s+(.+)$/';

    protected function lockFile(): string
    {
        return 'yarn.lock';
    }

    /**
     * @return Collection<int, Package|Approach>
     */
    public function scan(): Collection
    {
        $mappedItems = collect();
        $lockFilePath = $this->lockFilePath();

        $contents = $this->validateFile($lockFilePath, 'Yarn lock');
        if ($contents === null) {
            return $mappedItems;
        }

        $dependencies = [];
        $lines = explode("\n", $contents);
        $currentPackage = null;

        foreach ($lines as $line) {
            $line = trim($line);

            if ($line === '' || str_starts_with($line, '#')) {
                continue;
            }

            $packageName = $this->parsePackageHeader($line);

            if ($packageName !== null) {
                $currentPackage = $packageName;

                continue;
            }

            $version = $this->parseVersion($line);

            if ($currentPackage !== null && $version !== null) {
                $dependencies[$currentPackage] = $version;
                $currentPackage = null;
            }
        }

        // Yarn lock does not distinguish devDependencies :/
        $this->processDependencies($dependencies, $mappedItems, false);

        return $mappedItems;
    }

    private function parsePackageHeader(string $line): ?string
    {
        if (preg_match(self::YARN_V1_HEADER, $line, $matches)) {
            return $matches[2];
        }

        if (preg_match(self::YARN_V4_HEADER, $line, $matches)) {
            return $matches[1];
        }

        return null;
    }

    private function parseVersion(string $line): ?string
    {
        if (preg_match(self::YARN_V1_VERSION, $line, $matches)) {
            return $matches[1];
        }

        if (preg_match(self::YARN_V4_VERSION, $line, $matches)) {
            return trim($matches[1]);
        }

        return null;
    }
}
