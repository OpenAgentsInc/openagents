<?php

namespace App\Services;

class Finnhub
{
    public function __construct()
    {
        $config = Finnhub\Configuration::getDefaultConfiguration()->setApiKey('token', 'YOUR API KEY');
        $client = new Finnhub\Api\DefaultApi(
            new GuzzleHttp\Client(),
            $config
        );

        // Stock candles
        print_r($client->stockCandles('AAPL', 'D', 1590988249, 1591852249));

    }
}
