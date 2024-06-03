<?php

namespace App\Console\Commands;

use App\Enums\Currency;
use App\Http\Controllers\LnAddressController;
use App\Models\Payin;
use App\Models\User;
use Illuminate\Console\Command;
use Log;

class CheckPendingPayins extends Command
{
    protected $signature = 'payins:check';

    protected $description = 'Check pending payins and update statuses+balances';

    public function handle()
    {
        $pendingPayins = Payin::where('status', 'pending')->get();

        $controller = new LnAddressController();

        foreach ($pendingPayins as $payin) {
            $invoiceStatus = $controller->getInvoiceStatus($payin->payment_request);  // Or payment_hash if you store hashes

            if (! $invoiceStatus) {
                continue;
            }

            if ($invoiceStatus['settled']) {
                $payin->status = 'settled';
                $payin->save();

                $user = User::find($payin->user_id);

                // Assuming the payin amount is in BTC
                $currency = Currency::BTC;

                // Use the deposit method from the Payable trait to update the balance
                $user->deposit($payin->amount, $currency);

                Log::info('Payin settled: '.$payin->id);
            } else {
                Log::info('Payin still pending: '.$payin->id);
            }
        }

        return 0;
    }
}
