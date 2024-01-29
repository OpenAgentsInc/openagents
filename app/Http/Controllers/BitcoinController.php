<?php

namespace App\Http\Controllers;

use App\Services\Alby\Client as AlbyClient;
use App\Services\Bitcoin;
use Mauricius\LaravelHtmx\Http\HtmxResponse;

class BitcoinController extends Controller
{
    public function withdraw()
    {
        return view('withdraw', [
            'price' => Bitcoin::getUsdPrice(),
        ]);
    }

    public function initiate_withdrawal()
    {
        $this->validate(request(), [
            'amount' => 'required|integer|min:10',
        ]);

        $amount = request('amount');
        $address = 'atlantispleb@getalby.com'; // auth()->user()->lightning_address;

        // If no lightning_address, return an error message via HTMX
        if (! $address) {
            $errorMessage = 'You must set a lightning address before withdrawing.';

            return with(new HtmxResponse())
                ->renderFragment('withdraw-message', 'withdraw-message', compact('errorMessage'));
        }

        $withdrawal = auth()->user()->withdrawals()->create([
            'amount' => $amount,
            'lightning_address' => $address,
            'status' => 'pending',
        ]);

        // Generate an invoice
        $invoice = Bitcoin::requestInvoiceForLightningAddress([
            'lightning_address' => $address,
            'amount' => $amount * 1000, // convert to msat
            'memo' => 'OpenAgents Withdrawal',
        ]);

        $albyClient = new AlbyClient(env('ALBY_ACCESS_TOKEN'));
        $response = $albyClient->payInvoice($invoice['pr']);

        // If payment succeeds, mark withdrawal as completed
        if (is_string($response['payment_preimage'])) {
            $withdrawal->update([
                'status' => 'completed',
            ]);

            $successMessage = 'Withdrawal initiated!';

            return with(new HtmxResponse())
                ->renderFragment('withdraw-message', 'withdraw-message', compact('successMessage'));
        }

        // Handle failure case
        $errorMessage = 'Withdrawal failed. Please try again.';

        return with(new HtmxResponse())
            ->renderFragment('withdraw-message', 'withdraw-message', compact('errorMessage'));
    }

    public function old_initiate_withdrawal()
    {
        $this->validate(request(), [
            'amount' => 'required|integer|min:10',
        ]);

        $amount = request('amount');
        $address = 'atlantispleb@getalby.com'; // auth()->user()->lightning_address;

        // If no lightning_address, fail
        if (! $address) {
            $errorMessage = 'You must set a lightning address before withdrawing.';

            return with(new HtmxResponse())
                ->renderFragment('withdraw', 'withdraw-message', compact('errorMessage'));
        }

        $withdrawal = auth()->user()->withdrawals()->create([
            'amount' => $amount,
            'lightning_address' => $address,
            'status' => 'pending',
        ]);

        // Generate an invoice
        $invoice = Bitcoin::requestInvoiceForLightningAddress([
            'lightning_address' => $address,
            'amount' => $amount * 1000, // convert to msat
            'memo' => 'OpenAgents Withdrawal',
        ]);

        //$payment = Bitcoin::payInvoice($invoice['pr']);
        $albyClient = new AlbyClient(env('ALBY_ACCESS_TOKEN'));
        $response = $albyClient->payInvoice($invoice['pr']);

        // If payment succeeds, mark withdrawal as completed
        if (is_string($response['payment_preimage'])) {
            $withdrawal->update([
                'status' => 'completed',
            ]);

            $successMessage = 'Withdrawal initiated!';

            return with(new HtmxResponse())
                ->renderFragment('withdraw', 'withdraw-message', compact('successMessage'));
        }

        // Handle failure case
        $errorMessage = 'Withdrawal failed. Please try again.';

        return with(new HtmxResponse())
            ->renderFragment('withdraw', 'withdraw-message', compact('errorMessage'));
    }

    public function bitcoin()
    {
        return view('bitcoin', [
            'price' => Bitcoin::getUsdPrice(),
        ]);
    }

    public function bitcoinPrice()
    {
        return view('bitcoin-price', [
            'price' => Bitcoin::getUsdPrice(),
        ]);
    }

    public function sse()
    {
        return response()->stream(function () {
            while (true) {
                $price = Bitcoin::getUsdPrice();
                echo "data: BTCUSD \${$price}\n\n";
                ob_flush();
                flush();
                sleep(5);
            }
        }, 200, [
            'Content-Type' => 'text/event-stream',
            'Cache-Control' => 'no-cache',
            'Connection' => 'keep-alive',
        ]);
    }
}
