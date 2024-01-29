<?php

namespace App\Http\Controllers;

use App\Services\Bitcoin;

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
        $address = auth()->user()->lightning_address;

        // If no lightning_address, fail
        if (! $address) {
            return redirect()->route('withdraw')->with('error', 'You must set a lightning address before withdrawing.');
        }

        $withdrawal = auth()->user()->withdrawals()->create([
            'amount' => $amount,
            'lightning_address' => $address,
            'status' => 'pending',
        ]);
        // $response = Bitcoin::withdraw($amount, $address);

        return 'yo';
        //        if ($response->status === 'ok') {
        //return redirect()->route('withdraw')->with('success', 'Withdrawal initiated!');
        //}

        //        return redirect()->route('withdraw')->with('error', $response->message);
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
