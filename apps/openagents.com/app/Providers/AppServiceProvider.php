<?php

namespace App\Providers;

use App\Lightning\L402\InvoicePayer;
use App\Lightning\L402\InvoicePayers\FakeInvoicePayer;
use App\Lightning\L402\InvoicePayers\LndRestInvoicePayer;
use App\Lightning\L402\InvoicePayers\SparkWalletInvoicePayer;
use Carbon\CarbonImmutable;
use Illuminate\Support\Facades\Date;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\ServiceProvider;
use Illuminate\Validation\Rules\Password;
use PostHog\PostHog;
use RuntimeException;

class AppServiceProvider extends ServiceProvider
{
    /**
     * Register any application services.
     */
    public function register(): void
    {
        $this->app->singleton(InvoicePayer::class, function ($app) {
            $kind = (string) config('lightning.l402.invoice_payer', 'fake');

            return match ($kind) {
                'spark_wallet' => $app->make(SparkWalletInvoicePayer::class),
                'lnd_rest' => new LndRestInvoicePayer,
                'fake' => new FakeInvoicePayer,
                default => throw new RuntimeException('Unknown L402 invoice payer: '.$kind),
            };
        });
    }

    /**
     * Bootstrap any application services.
     */
    public function boot(): void
    {
        $this->configureDefaults();
        $this->configurePostHog();
    }

    /**
     * Initialize PostHog analytics.
     */
    protected function configurePostHog(): void
    {
        if (config('posthog.disabled')) {
            return;
        }

        $apiKey = config('posthog.api_key');

        if (! is_string($apiKey) || $apiKey === '') {
            return;
        }

        PostHog::init($apiKey, [
            'host' => config('posthog.host', 'https://us.i.posthog.com'),
            'debug' => config('posthog.debug', false),
        ]);
    }

    /**
     * Configure default behaviors for production-ready applications.
     */
    protected function configureDefaults(): void
    {
        Date::use(CarbonImmutable::class);

        DB::prohibitDestructiveCommands(
            app()->isProduction(),
        );

        Password::defaults(fn (): ?Password => app()->isProduction()
            ? Password::min(12)
                ->mixedCase()
                ->letters()
                ->numbers()
                ->symbols()
                ->uncompromised()
            : null
        );
    }
}
