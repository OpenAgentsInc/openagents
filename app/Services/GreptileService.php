<?php

namespace App\Services;

use GuzzleHttp\Client;
use Illuminate\Support\Facades\Log;

class GreptileService
{
    protected $apiKey;
    protected $apiUrl;

    public function __construct()
    {
        $this->apiKey = env('GREPTILE_API_KEY');
        $this->apiUrl = env('GREPTILE_API_URL');
    }

    public function searchCodebase($codebase, $branch, $query)
    {
        Log::info('Searching codebase', ['codebase' => $codebase, 'branch' => $branch, 'query' => $query]);

        $client = new Client();

        try {
            $response = $client->post($this->apiUrl, [
                'headers' => [
                    'Content-Type' => 'application/json',
                    'Authorization' => 'Bearer ' . $this->apiKey,
                ],
                'json' => [
                    'codebase' => $codebase,
                    'branch' => $branch,
                    'query' => $query,
                ],
            ]);

            $result = json_decode($response->getBody(), true);
            Log::info('Greptile search result', ['result' => $result]);

            return $result;
        } catch (\Exception $e) {
            Log::error('Error in Greptile search', ['error' => $e->getMessage()]);
            return ['error' => 'An error occurred while searching the codebase'];
        }
    }
}
