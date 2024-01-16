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
        return 'hi';
    }
}
