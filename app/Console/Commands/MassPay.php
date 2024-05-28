<?php

namespace App\Console\Commands;

use App\Enums\Currency;
use App\Models\User;
use App\Services\PaymentService;
use Illuminate\Console\Command;

class MassPay extends Command
{
    protected $signature = 'masspay';

    protected $description = 'Pay all users';

    public function handle(PaymentService $paymentService): void
    {
        $recipients = User::all();

        // Split into two: pro users and non-pro users
        $proRecipients = $recipients->filter(fn (User $user) => $user->isPro());
        $nonProRecipients = $recipients->filter(fn (User $user) => ! $user->isPro());

        $currency = Currency::BTC;

        $msats = 500 * 1000; // Pay 500 sats to non-pro users
        $msats_pro = 2500 * 1000; // Pay 2500 sats to pro users

        // Instantiate PaymentService and make the payment
        $paymentService->paySystemBonusToMultipleRecipients($nonProRecipients, $msats, $currency, 'Bonus test');
        $paymentService->paySystemBonusToMultipleRecipients($proRecipients, $msats_pro, $currency, 'Bonus test (5x extra for pro!)');

        $this->info('Mass payment successful.');
    }
}
