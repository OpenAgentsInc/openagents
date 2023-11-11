<?php

namespace App\Services;

use Illuminate\Support\Facades\Http;

class Vectara
{
    private $baseUrl = 'https://api.vectara.io';
    private $authUrl;
    private $clientId;
    private $clientSecret;

    public function __construct()
    {
        $this->authUrl = "https://vectara-prod-352100613.auth.us-west-2.amazoncognito.com";
        $this->clientId = "2vg8b501te1f5oppsj6nttrfg";
        $this->clientSecret = "ug92rc910hme7q4qc03rlcas7fpghbmmt4nekqjmj14oq96dd7a";
    }

    public function getJwtToken()
    {
        $url = $this->authUrl . '/oauth2/token';
        $encoded = base64_encode($this->clientId . ':' . $this->clientSecret);

        $fields = [
            'grant_type' => 'client_credentials',
            'client_id' => $this->clientId,
        ];

        $response = Http::asForm()->withHeaders([
            'Authorization' => 'Basic ' . $encoded
        ])->post($url, $fields);

        return $response->successful() ? $response->json()['access_token'] : null;
    }

    public function createCorpus($customer_id, $corpusData)
    {
        $jwtToken = $this->getJwtToken();
        if (!$jwtToken) {
            return ['ok' => false, 'error' => 'Failed to obtain JWT token'];
        }

        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
            'Authorization' => 'Bearer ' . $jwtToken,
            'customer-id' => "352100613",
            'grpc-timeout' => '30S'
        ])->post($this->baseUrl . '/v1/create-corpus', [
            'corpus' => $corpusData
        ]);

        return $response->successful()
               ? ['ok' => true, 'data' => $response->json()]
               : ['ok' => false, 'error' => $response->body()];
    }
}
