<?php

namespace App\Services;

use Illuminate\Support\Facades\Http;

class Bitcoin
{
    public static function getUsdPrice(): float
    {
        return cache()->remember('bitcoin_price', 5, function () {
            if (app()->environment('testing')) {
                return 10000.00;
            }

            $fmpKey = env('FMP_API_KEY');
            $url = "https://financialmodelingprep.com/api/v3/quote/BTCUSD?apikey={$fmpKey}";
            $response = Http::get($url)->json();
            return $response[0]['price'] ?? 0;
        });
    }
}
