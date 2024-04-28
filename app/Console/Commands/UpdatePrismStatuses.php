<?php

namespace App\Console\Commands;

use App\Models\PrismSinglePayment;
use App\Services\PrismService;
use Illuminate\Console\Command;

class UpdatePrismStatuses extends Command
{
    protected $signature = 'prism:update';

    protected $description = 'Update prism statuses';

    public function handle()
    {
        // Fetch all PrismSinglePayments where status is 'sending'
        $payments = PrismSinglePayment::where('status', 'sending')->get();

        dump('Updating '.$payments->count().' payments...');

        $prismService = new PrismService();

        // loop through them
        foreach ($payments as $payment) {
            // Assume we have a PrismService class with a method to check the status of a payment
            $wat = $prismService->getTransactionDetails($payment->payment_id);

            // If status is different than the one we have, update it
            if (is_array($wat) && isset($wat['status']) && ($wat['status'] !== $payment->status)) {
                $this->info('Updating payment '.$payment->payment_id.' to status '.$wat['status']);
                $payment->update(['status' => $wat['status']]);
            }

            sleep(1);
        }
    }
}
