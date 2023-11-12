<?php

namespace App\Services;

use Illuminate\Http\UploadedFile;
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

    public function createCorpus($corpusData)
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

    public function upload($corpus_id, UploadedFile $file, $doc_metadata = [])
    {
        $jwtToken = $this->getJwtToken();
        if (!$jwtToken) {
            return ['ok' => false, 'error' => 'Failed to obtain JWT token'];
        }

        $response = Http::withHeaders([
            'Authorization' => 'Bearer ' . $jwtToken,
            'customer-id' => "352100613",
            'grpc-timeout' => '30S'
        ])->attach(
            'file',
            fopen($file->getRealPath(), 'r'),
            $file->getClientOriginalName()
        )->post($this->baseUrl . '/v1/upload', [
            'c' => "352100613",
            'o' => $corpus_id,
            // 'doc_metadata' => json_encode($doc_metadata)
        ]);

        return $response->successful()
               ? ['ok' => true, 'data' => $response->json()]
               : ['ok' => false, 'error' => $response->body()];
    }

    public function query($corpus_id, $query, $numResults = 10)
    {
        $jwtToken = $this->getJwtToken();
        if (!$jwtToken) {
            return ['ok' => false, 'error' => 'Failed to obtain JWT token'];
        }

        $query_data = [
            'query' => $query,
            'numResults' => $numResults,
            'summary' => [
              [
                  'summarizerPromptName' => 'vectara-summary-ext-v1.2.0', // Replace with the actual summarizer+prompt name
                  'maxSummarizedResults' => 1,
                  'responseLang' => 'auto', // Or specify a language code if required
              ],
            ],
            'corpusKey' => [
                [
                    'customerId' => "352100613",
                    'corpusId' => $corpus_id,
                ],
            ]
        ];

        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
            'Authorization' => 'Bearer ' . $jwtToken,
            'customer-id' => "352100613"
        ])->post($this->baseUrl . '/v1/query', ['query' => [$query_data]]);

        if ($response->successful()) {
            $responseData = $response->json();

            // Check if summary is available and has content
            $summaryText = "";
            if (!empty($responseData['responseSet'][0]['summary'][0]['text'])) {
                $summaryText = $responseData['responseSet'][0]['summary'][0]['text'];
            }

            return ['ok' => true, 'data' => $responseData, 'summary' => $summaryText];
        } else {
            return ['ok' => false, 'error' => $response->body()];
        }
    }
}
