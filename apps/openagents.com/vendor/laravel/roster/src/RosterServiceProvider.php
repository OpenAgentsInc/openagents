<?php

namespace Laravel\Roster;

use Illuminate\Support\ServiceProvider;

class RosterServiceProvider extends ServiceProvider
{
    /**
     * Bootstrap any package services.
     */
    public function boot(): void
    {
        $this->registerCommands();
    }

    /**
     * Register the package's commands.
     */
    protected function registerCommands(): void
    {
        if ($this->app->runningInConsole()) {
            $this->commands([
                Console\ScanCommand::class,
            ]);
        }
    }
}
