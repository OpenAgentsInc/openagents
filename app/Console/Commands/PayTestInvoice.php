<?php

namespace App\Console\Commands;

use App\Models\User;
use App\Services\PaymentService;
use Auth;
use Illuminate\Console\Command;

class PayTestInvoice extends Command
{
    protected $signature = 'pay-invoice {bolt11}';

    protected $description = 'Command description';

    /**
     * Execute the console command.
     */
    public function handle(PaymentService $paymentService)
    {
        $bolt11 = $this->argument('bolt11');
        $this->info("Paying invoice: $bolt11");

        $user = User::where('username', 'AtlantisPleb')->first();
        Auth::login($user);

        $response = $paymentService->processPaymentRequest($bolt11);

        print_r($response);
        //        $this->info($response['message'] ?? 'donno');
    }
}
