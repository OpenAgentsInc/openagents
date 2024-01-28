<?php

namespace App\Services;

use App\Services\Alby\Client;

class L402
{
    public function __construct()
    {
        $this->wallet = new Client(env('ALBY_ACCESS_TOKEN'));
    }

    public function sendRequest($url, $l402Token = null)
    {
        $headers = [];
        if ($l402Token) {
            $headers['Authorization'] = 'L402 '.$l402Token;
        }

        return Http::withHeaders($headers)->get($url);
    }

    public function parseAuthHeader($authHeader)
    {
        return explode(':', str_replace('L402 ', '', $authHeader));
    }

    public function assembleL402Token($macaroon, $preimage)
    {
        return $macaroon.':'.$preimage;
    }
}
