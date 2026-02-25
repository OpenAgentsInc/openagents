<?php

namespace Laravel\Roster;

use Illuminate\Support\Collection;

/**
 * @extends Collection<int, Package>
 */
class PackageCollection extends Collection
{
    public function dev(): static
    {
        return $this->filter(fn (Package $package) => $package->isDev());
    }

    public function production(): static
    {
        return $this->filter(fn (Package $package) => ! $package->isDev());
    }
}
