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

    public function createUser($lightningAddress = null)
    {
        $userData = [];
        if ($lightningAddress) {
            $userData['lnAddress'] = $lightningAddress;
        }

        $response = Http::withToken($this->apiKey)
            ->post("{$this->baseUrl}/user", $userData);

        return $response->json(); // Expecting to get back a user ID
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
                'amount' => $amount,
                'currency' => $currency,
                'prism' => $recipients,
            ]);

        return $response->json();
    }
}
