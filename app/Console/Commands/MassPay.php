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

    public function handle(PaymentService $paymentService)
    {
        $recipients = User::whereIn('id', [1, 2, 3, 4])->get(); // Replace with actual recipient IDs
        $amount = 1000; // Amount in satoshis or required denomination
        $currency = Currency::BTC;

        // Instantiate PaymentService and make the payment
        $paymentService->paySystemBonusToMultipleRecipients($recipients, $amount, $currency);

        $this->info('Mass payment successful.');
    }
}
