<?php

namespace App\Services;

use Illuminate\Support\Facades\Http;

class PrismService
{
    protected $baseUrl = 'https://api.makeprisms.com/v0';

    protected $apiKey; // Assume you've stored your API key somewhere secure

    public function __construct()
    {
        $this->apiKey = env('PRISM_API_KEY');
    }

    public function createUser($lnAddress = null, $nwcConnection = null)
    {
        $payload = [];

        if (! is_null($lnAddress)) {
            $payload['lnAddress'] = $lnAddress;
        }

        if (! is_null($nwcConnection)) {
            // Assuming $nwcConnection is an array with the necessary fields
            $payload['nwcConnection'] = $nwcConnection;
        } else {
            // Use the NWC_URL from .env if no nwcConnection is explicitly passed
            $nwcUrl = env('NWC_URL');
            $payload['nwcConnection'] = [
                'nwcUrl' => $nwcUrl,
                'connectorType' => 'nwc.alby',
                'connectorName' => 'bitcoin-connect',
            ];
        }

        $response = Http::withToken($this->apiKey)
            ->post("{$this->baseUrl}/user", $payload);

        return $response->json();
    }

    public function updateUserLnAddress($userId, $lightningAddress)
    {
        $response = Http::withToken($this->apiKey)
            ->patch("{$this->baseUrl}/user/{$userId}", [
                'lnAddress' => $lightningAddress,
            ]);

        return $response->json();
    }

    public function sendPayment($amount, $currency, array $recipients)
    {
        // Assuming $recipients is an array of lightning addresses
        $response = Http::withToken($this->apiKey)
            ->post("{$this->baseUrl}/payment/prism", [
                'senderId' => '67c2cc15-d90f-4af5-b16f-06cebd9e8e5d', // atlantispleb
                //                'senderId' => '1e90c130-8bce-4aa8-abd0-92329d57fafe', // openagents
                'amount' => $amount,
                'currency' => $currency,
                'prism' => $recipients,
            ]);

        return $response->json();
    }

    public function getTransactionDetails($transactionId)
    {
        // Replace with the actual endpoint for fetching transaction details
        // Assuming the endpoint follows the pattern /transaction/{transactionId}
        $response = Http::withToken($this->apiKey)
            ->get("{$this->baseUrl}/payment/{$transactionId}");

        return $response->json();
    }
}
