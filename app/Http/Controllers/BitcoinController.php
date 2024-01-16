<?php

namespace App\Http\Controllers;

use App\Services\Bitcoin;

class BitcoinController extends Controller
{
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
