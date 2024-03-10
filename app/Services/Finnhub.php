<?php

namespace App\Services;

use Finnhub\Api\DefaultApi;
use Finnhub\Configuration;
use GuzzleHttp\Client;

class Finnhub
{
    public function __construct()
    {
        $config = Configuration::getDefaultConfiguration()->setApiKey('token', env('FINNHUB_API_KEY'));
        $client = new DefaultApi(
            new Client(),
            $config
        );

        // Stock candles
        print_r($client->symbolSearch('AAPL'));

    }
}
