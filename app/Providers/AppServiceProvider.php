<?php

namespace App\Providers;

use App\Models\User;
use App\Services\LocalLogger;
use App\Services\StreamService;
use Illuminate\Support\Facades\Gate;
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
        Gate::define('viewApiDocs', function (User $user) {
            // return in_array($user->email, ['admin@app.com']);
            return true;
        });
    }
}
