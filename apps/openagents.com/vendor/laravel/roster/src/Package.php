<?php

namespace Laravel\Roster;

use Laravel\Roster\Enums\Packages;

class Package
{
    protected bool $direct = false;

    protected string $constraint = '';

    public function __construct(protected Packages $package, protected string $packageName, protected string $version, protected bool $dev = false) {}

    public function setDev(bool $dev = true): self
    {
        $this->dev = $dev;

        return $this;
    }

    public function setDirect(bool $direct = true): self
    {
        $this->direct = $direct;

        return $this;
    }

    public function setConstraint(string $constraint = ''): self
    {
        $this->constraint = $constraint;

        return $this;
    }

    public function name(): string
    {
        return $this->package->name;
    }

    public function package(): Packages
    {
        return $this->package;
    }

    public function version(): string
    {
        return $this->version;
    }

    public function direct(): bool
    {
        return $this->direct;
    }

    public function indirect(): bool
    {
        return ! $this->direct;
    }

    public function constraint(): string
    {
        return $this->constraint;
    }

    public function majorVersion(): string
    {
        return explode('.', $this->version)[0];
    }

    public function isDev(): bool
    {
        return $this->dev;
    }

    public function rawName(): string
    {
        return $this->packageName;
    }
}
