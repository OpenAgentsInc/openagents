<?php

declare(strict_types=1);

namespace Pest\Profanity\Support;

use PHPUnit\TextUI\CliArguments\Builder;
use PHPUnit\TextUI\CliArguments\XmlConfigurationFileFinder;
use PHPUnit\TextUI\Configuration\FilterDirectory;
use PHPUnit\TextUI\XmlConfiguration\DefaultConfiguration;
use PHPUnit\TextUI\XmlConfiguration\Loader;

/**
 * @internal
 */
final class ConfigurationSourceDetector
{
    /**
     * Detects the "source" of the configuration.
     *
     * @return array<int, string>
     */
    public static function detect(): array
    {
        $cliConfiguration = (new Builder)->fromParameters([]);
        $configurationFile = (new XmlConfigurationFileFinder)->find($cliConfiguration);
        $xmlConfiguration = DefaultConfiguration::create();

        if (is_string($configurationFile)) {
            $xmlConfiguration = (new Loader)->load($configurationFile);
        }

        return array_map(
            fn (FilterDirectory $directory): string => (string) realpath($directory->path()),
            $xmlConfiguration->source()->includeDirectories()->asArray()
        );
    }
}
