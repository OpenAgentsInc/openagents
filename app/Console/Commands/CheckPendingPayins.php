<?php

namespace App\Console\Commands;

use App\Enums\Currency;
use App\Http\Controllers\LnAddressController;
use App\Models\Payin;
use App\Models\User;
use Carbon\Carbon;
use Illuminate\Console\Command;
use Log;

class CheckPendingPayins extends Command
{
    protected $signature = 'payins:check';

    protected $description = 'Check pending payins and update statuses+balances';

    public function handle()
    {
        $pendingPayins = Payin::where('status', 'pending')
            ->where('created_at', '>', now()->subDay())
            ->get();

        $controller = new LnAddressController();
        $delayLimit = 21600; // 6 hours

        foreach ($pendingPayins as $payin) {

            $last_check = $payin->last_check;
            $retry = $payin->retry_check;

            $expectedDelaySeconds = min(10 * (2 ** $retry), $delayLimit);
            $last_check = $last_check instanceof Carbon ? $last_check : new Carbon($last_check);

            if (now()->timestamp - $last_check->timestamp < $expectedDelaySeconds) {
                continue;
            }

            $payin->last_check = now();
            $payin->retry_check = $retry + 1;
            $payin->save();

            $invoiceStatus = $controller->getInvoiceStatus($payin->payment_hash);

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
                $nextExpectedDelaySeconds = min(10 * (2 ** ($retry + 1)), $delayLimit);
                Log::info('Payin still pending: '.$payin->id.' next attempt in '.$nextExpectedDelaySeconds.' seconds');
            }
        }

        return 0;
    }
}
