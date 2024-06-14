<?php

namespace App\Providers;

use App\Models\User;
use App\Services\LocalLogger;
use App\Services\PaymentService;
use App\Services\StreamService;
use Illuminate\Support\Facades\Gate;
use Illuminate\Support\ServiceProvider;
use Inertia\Inertia;

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

        $this->app->singleton(PaymentService::class, function ($app) {
            return new PaymentService();
        });
    }

    /**
     * Bootstrap any application services.
     */
    public function boot(): void
    {
        Inertia::setRootView('inertia');

        Gate::define('viewApiDocs', function (User $user) {
            // return in_array($user->email, ['admin@app.com']);
            return true;
        });
    }
}
