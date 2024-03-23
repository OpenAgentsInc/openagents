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

    public function sendPayment($amount, $currency, array $recipients)
    {
        // Assuming $recipients is an array of lightning addresses
        $response = Http::withToken($this->apiKey)
            ->post("{$this->baseUrl}/payment/prism", [
                'amount' => $amount,
                'currency' => $currency,
                'prism' => $recipients,
            ]);

        return $response->json();
    }
}
