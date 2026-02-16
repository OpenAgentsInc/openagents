<?php

namespace App\Providers;

use App\Lightning\L402\InvoicePayer;
use App\Lightning\L402\InvoicePayers\FakeInvoicePayer;
use App\Lightning\L402\InvoicePayers\LndRestInvoicePayer;
use Carbon\CarbonImmutable;
use Illuminate\Support\Facades\Date;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\ServiceProvider;
use Illuminate\Validation\Rules\Password;
use RuntimeException;

class AppServiceProvider extends ServiceProvider
{
    /**
     * Register any application services.
     */
    public function register(): void
    {
        $this->app->singleton(InvoicePayer::class, function () {
            $kind = (string) config('lightning.l402.invoice_payer', 'fake');

            return match ($kind) {
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
