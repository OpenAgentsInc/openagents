<?php

declare(strict_types=1);

namespace Laravel\Boost\Console;

use Illuminate\Console\Command;
use Illuminate\Support\Facades\Artisan;
use Symfony\Component\Console\Attribute\AsCommand;

#[AsCommand('boost:mcp', 'Starts Laravel Boost (usually from mcp.json)')]
class StartCommand extends Command
{
    public function handle(): int
    {
        return Artisan::call('mcp:start laravel-boost');
    }
}
