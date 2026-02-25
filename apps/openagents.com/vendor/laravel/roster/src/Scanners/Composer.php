<?php

namespace Laravel\Roster\Scanners;

use Illuminate\Support\Collection;
use Illuminate\Support\Facades\Log;
use Laravel\Roster\Approach;
use Laravel\Roster\Enums\Approaches;
use Laravel\Roster\Enums\Packages;
use Laravel\Roster\Package;

class Composer
{
    /**
     * Map of composer package names to enums
     *
     * @var array<string, Packages|Approaches|array<int, Packages|Approaches>|null>
     */
    protected array $map = [
        'filament/filament' => Packages::FILAMENT,
        'inertiajs/inertia-laravel' => [Packages::INERTIA, Packages::INERTIA_LARAVEL],
        'larastan/larastan' => Packages::LARASTAN,
        'laravel/boost' => Packages::BOOST,
        'laravel/breeze' => Packages::BREEZE,
        'laravel/cashier' => Packages::CASHIER,
        'laravel/dusk' => Packages::DUSK,
        'laravel/envoy' => Packages::ENVOY,
        'laravel/folio' => Packages::FOLIO,
        'laravel/fortify' => Packages::FORTIFY,
        'laravel/framework' => Packages::LARAVEL,
        'laravel/horizon' => Packages::HORIZON,
        'laravel/mcp' => Packages::MCP,
        'laravel/nightwatch' => Packages::NIGHTWATCH,
        'laravel/nova' => Packages::NOVA,
        'laravel/octane' => Packages::OCTANE,
        'laravel/pail' => Packages::PAIL,
        'laravel/passport' => Packages::PASSPORT,
        'laravel/pennant' => Packages::PENNANT,
        'laravel/pint' => Packages::PINT,
        'laravel/prompts' => Packages::PROMPTS,
        'laravel/pulse' => Packages::PULSE,
        'laravel/reverb' => Packages::REVERB,
        'laravel/sail' => Packages::SAIL,
        'laravel/sanctum' => Packages::SANCTUM,
        'laravel/scout' => Packages::SCOUT,
        'laravel/socialite' => Packages::SOCIALITE,
        'laravel/telescope' => Packages::TELESCOPE,
        'laravel/wayfinder' => [Packages::WAYFINDER, Packages::WAYFINDER_LARAVEL],
        'livewire/flux' => Packages::FLUXUI_FREE,
        'livewire/flux-pro' => Packages::FLUXUI_PRO,
        'livewire/livewire' => Packages::LIVEWIRE,
        'livewire/volt' => Packages::VOLT,
        'pestphp/pest' => Packages::PEST,
        'phpunit/phpunit' => Packages::PHPUNIT,
        'rector/rector' => Packages::RECTOR,
        'statamic/cms' => Packages::STATAMIC,
        'tightenco/ziggy' => Packages::ZIGGY,
    ];

    /** @var array<string, array{constraint: string, isDev: bool}> */
    protected array $directPackages = [];

    /**
     * @param  string  $path  - composer.lock
     */
    public function __construct(protected string $path) {}

    /**
     * @return \Illuminate\Support\Collection<int, \Laravel\Roster\Package|\Laravel\Roster\Approach>
     */
    public function scan(): Collection
    {
        $mappedItems = collect([]);

        if (! file_exists($this->path)) {
            Log::warning('Failed to scan Composer: '.$this->path);

            return $mappedItems;
        }

        if (! is_readable($this->path)) {
            Log::warning('File not readable: '.$this->path);

            return $mappedItems;
        }

        $contents = file_get_contents($this->path);
        if ($contents === false) {
            Log::warning('Failed to read Composer: '.$this->path);

            return $mappedItems;
        }

        $json = json_decode($contents, true);
        if (json_last_error() !== JSON_ERROR_NONE || ! is_array($json)) {
            Log::warning('Failed to decode Composer: '.$this->path.'. '.json_last_error_msg());

            return $mappedItems;
        }

        if (! array_key_exists('packages', $json)) {
            Log::warning('Malformed composer.lock');

            return $mappedItems;
        }

        $this->directPackages = $this->direct();
        $packages = $json['packages'] ?? [];
        $devPackages = $json['packages-dev'] ?? [];

        $this->processPackages($packages, $mappedItems, false);
        $this->processPackages($devPackages, $mappedItems, true);

        return $mappedItems;
    }

    /**
     * Returns direct dependencies as defined in composer.json
     *
     * @return array<string, array{constraint: string, isDev: bool}>
     * */
    protected function direct(): array
    {
        $packages = [];
        $filename = realpath(dirname($this->path)).DIRECTORY_SEPARATOR.'composer.json';
        if (file_exists($filename) === false || is_readable($filename) === false) {
            return $packages;
        }

        $json = file_get_contents($filename);
        if ($json === false) {
            return $packages;
        }

        $json = json_decode($json, true);
        if (json_last_error() !== JSON_ERROR_NONE || ! is_array($json)) {
            return $packages;
        }

        foreach ((array) ($json['require'] ?? []) as $name => $constraint) {
            $packages[$name] = [
                'constraint' => $constraint,
                'isDev' => false,
            ];
        }

        foreach ((array) ($json['require-dev'] ?? []) as $name => $constraint) {
            $packages[$name] = [
                'constraint' => $constraint,
                'isDev' => true,
            ];
        }

        return $packages;
    }

    /**
     * Process packages and add them to the mapped items collection
     *
     * @param  array<int, array<string, string>>  $packages
     * @param  Collection<int, Package|Approach>  $mappedItems
     * @return Collection<int, Package|Approach>
     */
    private function processPackages(array $packages, Collection $mappedItems, bool $isDev): Collection
    {
        foreach ($packages as $package) {
            $packageName = $package['name'] ?? '';
            $version = $package['version'] ?? '';
            $mappedPackage = $this->map[$packageName] ?? null;
            $direct = false;
            $constraint = $version;

            if (is_null($mappedPackage)) {
                continue;
            }

            if (! is_array($mappedPackage)) {
                $mappedPackage = [$mappedPackage];
            }

            if (array_key_exists($packageName, $this->directPackages) === true) {
                $direct = true;
                $constraint = $this->directPackages[$packageName]['constraint'];
            }

            foreach ($mappedPackage as $mapped) {
                $niceVersion = preg_replace('/[^0-9.]/', '', $version) ?? '';
                $mappedItems->push(match (get_class($mapped)) {
                    Packages::class => (new Package($mapped, $packageName, $niceVersion, $isDev))->setDirect($direct)->setConstraint($constraint),
                    Approaches::class => new Approach($mapped),
                    default => throw new \InvalidArgumentException('Unsupported mapping')
                });
            }
        }

        return $mappedItems;
    }
}
