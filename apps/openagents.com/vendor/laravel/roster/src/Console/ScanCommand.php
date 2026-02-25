<?php

namespace Laravel\Roster\Console;

use Illuminate\Console\Command;
use Laravel\Roster\Roster;

class ScanCommand extends Command
{
    /**
     * The name and signature of the console command.
     *
     * @var string
     */
    protected $signature = 'roster:scan {directory}';

    protected $description = 'Detect packages & approaches in use and output as JSON';

    public function handle(): int
    {
        $directory = $this->argument('directory');
        if (! is_string($directory)) {
            $this->error('Pass a directory');

            return self::FAILURE;
        }

        if (! is_dir($directory) || ! is_readable($directory)) {
            $this->error("Directory '{$directory}' isn't a directory");

            return self::FAILURE;
        }

        $roster = Roster::scan($directory);
        $this->line($roster->json());

        return self::SUCCESS;
    }
}
