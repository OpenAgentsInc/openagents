<?php

namespace App\Services;

use GuzzleHttp\Client;
use Illuminate\Support\Facades\Log;

class GreptileService
{
    protected $apiToken;
    protected $githubToken;
    protected $apiUrl = 'https://api.greptile.com/v2/search';

    public function __construct()
    {
        $this->apiToken = env('GREPTILE_API_KEY');
        $this->githubToken = env('GITHUB_TOKEN');
        Log::info('GreptileService initialized', [
            'apiUrl' => $this->apiUrl,
            'apiTokenSet' => !empty($this->apiToken),
            'githubTokenSet' => !empty($this->githubToken)
        ]);
    }

    public function searchCodebase($query, $repositories, $sessionId = null, $stream = false)
    {
        Log::info('Searching codebase', [
            'query' => $query,
            'repositories' => $repositories,
            'sessionId' => $sessionId,
            'stream' => $stream
        ]);

        if (empty($this->apiToken) || empty($this->githubToken)) {
            Log::error('API tokens not set', [
                'greptileTokenSet' => !empty($this->apiToken),
                'githubTokenSet' => !empty($this->githubToken)
            ]);
            return ['error' => 'API tokens not properly configured'];
        }

        $client = new Client();

        try {
            Log::info('Sending request to Greptile API', ['url' => $this->apiUrl]);

            $response = $client->post($this->apiUrl, [
                'headers' => [
                    'Content-Type' => 'application/json',
                    'Authorization' => 'Bearer ' . $this->apiToken,
                    'X-GitHub-Token' => $this->githubToken,
                ],
                'json' => [
                    'query' => $query,
                    'repositories' => $repositories,
                    'sessionId' => $sessionId,
                    'stream' => $stream,
                ],
            ]);

            $statusCode = $response->getStatusCode();
            $result = json_decode($response->getBody(), true);

            Log::info('Greptile API response received', [
                'statusCode' => $statusCode,
                'resultSize' => strlen(json_encode($result))
            ]);

            if ($statusCode !== 200) {
                Log::error('Greptile API returned non-200 status code', [
                    'statusCode' => $statusCode,
                    'result' => $result
                ]);
                return ['error' => 'Greptile API returned status code ' . $statusCode];
            }

            Log::info('Greptile search completed successfully');
            return $result;
        } catch (\Exception $e) {
            Log::error('Error in Greptile search', [
                'error' => $e->getMessage(),
                'trace' => $e->getTraceAsString()
            ]);
            return ['error' => 'An error occurred while searching the codebase: ' . $e->getMessage()];
        }
    }
}
