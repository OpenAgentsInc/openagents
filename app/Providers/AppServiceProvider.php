<?php

namespace App\Providers;

use App\Services\LocalLogger;
use App\Services\StreamService;
use Illuminate\Support\ServiceProvider;

class AppServiceProvider extends ServiceProvider
{
    /**
     * Register any application services.
     */
    public function register()
    {
        $this->app->singleton(LocalLogger::class, function ($app) {
            return new LocalLogger();
        });

        $this->app->singleton(StreamService::class, function ($app) {
            return new StreamService();
        });
    }

    /**
     * Bootstrap any application services.
     */
    public function boot(): void
    {
        //
    }
}
