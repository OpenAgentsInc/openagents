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

        $msats = 50 * 1000; // Pay 50 sats to non-pro users
        $msats_pro = 250 * 1000; // Pay 250 sats to pro users

        // Instantiate PaymentService and make the payment
        $paymentService->paySystemBonusToMultipleRecipients($nonProRecipients, $msats, $currency, 'A little bonus');
        $paymentService->paySystemBonusToMultipleRecipients($proRecipients, $msats_pro, $currency, 'A little bonus (5x extra for pro!)');

        $this->info('Mass payment successful.');
    }
}
