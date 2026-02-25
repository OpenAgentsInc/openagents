<?php

namespace Laravel\Roster;

use Illuminate\Support\Collection;
use Laravel\Roster\Enums\Approaches;
use Laravel\Roster\Enums\NodePackageManager;
use Laravel\Roster\Enums\Packages;
use Laravel\Roster\Scanners\Composer;
use Laravel\Roster\Scanners\DirectoryStructure;
use Laravel\Roster\Scanners\PackageLock;

/**
 * Package and approach detection service for Laravel projects.
 *
 * Scans composer.lock, package-lock.json, and directory structure to identify
 * packages and development approaches in use.
 */
class Roster
{
    /**
     * @var Collection<int, \Laravel\Roster\Approach>
     */
    protected Collection $approaches;

    protected PackageCollection $packages;

    protected ?NodePackageManager $nodePackageManager = null;

    public function __construct()
    {
        $this->approaches = collect();
        $this->packages = new PackageCollection;
    }

    /**
     * @throws \InvalidArgumentException
     */
    public function add(Package|Approach $item): self
    {
        return match (get_class($item)) {
            Package::class => $this->addPackage($item),
            Approach::class => $this->addApproach($item),
            default => throw new \InvalidArgumentException('Unexpected match value'),
        };
    }

    public function uses(Packages|Approaches $item): bool
    {
        return $this->findItem($item) !== null;
    }

    /**
     * @throws \InvalidArgumentException
     */
    public function usesVersion(Packages $package, string $version, string $operator = '='): bool
    {
        if (! preg_match('/[0-9]{1,}\.[0-9]{1,}\.[0-9]{1,}/', $version)) {
            throw new \InvalidArgumentException('SEMVER required');
        }

        $validOperators = ['<', '<=', '>', '>=', '==', '=', '!=', '<>'];
        if (! in_array($operator, $validOperators)) {
            throw new \InvalidArgumentException('Invalid operator');
        }

        $package = $this->findItem($package);
        if (is_null($package)) {
            return false;
        }

        /** @var \Laravel\Roster\Package $package */
        return version_compare($package->version(), $version, $operator);
    }

    protected function findItem(Packages|Approaches $item): Package|Approach|null
    {
        return match (get_class($item)) {
            Packages::class => $this->package($item),
            Approaches::class => $this->approach($item),
            default => null,
        };
    }

    protected function addPackage(Package $package): self
    {
        $this->packages->push($package);

        return $this;
    }

    protected function addApproach(Approach $approach): self
    {
        $this->approaches->push($approach);

        return $this;
    }

    /**
     * @return Collection<int, \Laravel\Roster\Approach>
     */
    public function approaches(): Collection
    {
        return $this->approaches;
    }

    public function packages(): PackageCollection
    {
        return $this->packages;
    }

    public function package(Packages $package): ?Package
    {
        return $this->packages->first(fn (Package $item) => $item->package()->value === $package->value);
    }

    public function approach(Approaches $approach): ?Approach
    {
        return $this->approaches->first(fn (Approach $item) => $item->approach()->value === $approach->value);
    }

    public function nodePackageManager(): ?NodePackageManager
    {
        return $this->nodePackageManager;
    }

    public function json(): string
    {
        return json_encode([
            'approaches' => $this->approaches->map(fn (Approach $approach) => [
                'name' => $approach->name(),
            ])->toArray(),
            'packages' => $this->packages->map(fn (Package $package) => [
                'name' => $package->name(),
                'version' => $package->version(),
            ])->toArray(),
            'nodePackageManager' => $this->nodePackageManager?->value,
        ], JSON_PRETTY_PRINT) ?: '{}';
    }

    public static function scan(?string $basePath = null): self
    {
        $roster = new self;
        $basePath = ($basePath ?? base_path()).DIRECTORY_SEPARATOR;

        (new Composer($basePath.'composer.lock'))
            ->scan()
            ->each(fn ($item) => $roster->add($item));

        $packageLock = new PackageLock($basePath);

        $packageLock->scan()
            ->each(fn ($item) => $roster->add($item));

        (new DirectoryStructure($basePath))
            ->scan()
            ->each(fn ($item) => $roster->add($item));

        $roster->nodePackageManager = $packageLock->detect();

        return $roster;
    }
}
