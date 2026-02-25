<?php

declare(strict_types=1);

namespace Laravel\Boost\Install\Detection;

use Illuminate\Support\Facades\Process;
use Laravel\Boost\Install\Contracts\DetectionStrategy;
use Laravel\Boost\Install\Enums\Platform;

class CommandDetectionStrategy implements DetectionStrategy
{
    public function detect(array $config, ?Platform $platform = null): bool
    {
        if (! isset($config['command'])) {
            return false;
        }

        return Process::run($config['command'])->successful();
    }
}
