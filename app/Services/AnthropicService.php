<?php

namespace App\Services;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;
use Illuminate\Support\Facades\Log;

class AnthropicService
{
    protected $apiKey;
    protected $apiUrl = 'https://api.anthropic.com/v1/messages';

    public function __construct()
    {
        $this->apiKey = env('ANTHROPIC_API_KEY');
    }

    public function streamResponse($messages, $callback)
    {
        Log::info('Starting streamResponse', ['messageCount' => count($messages), 'messages' => $messages]);

        $client = new Client();

        try {
            $requestBody = [
                'model' => 'claude-3-5-sonnet-20240620',
                'max_tokens' => 1024,
                'messages' => $messages,
                'stream' => true,
            ];

            Log::info('Sending request to Anthropic API', ['requestBody' => $requestBody]);

            $response = $client->post($this->apiUrl, [
                'headers' => [
                    'Content-Type' => 'application/json',
                    'x-api-key' => $this->apiKey,
                    'anthropic-version' => '2023-06-01',
                ],
                'json' => $requestBody,
                'stream' => true,
            ]);

            $body = $response->getBody();

            while (!$body->eof()) {
                $line = $body->read(1024);
                $lines = explode("\n", $line);

                foreach ($lines as $line) {
                    if (str_starts_with($line, 'data: ')) {
                        $data = json_decode(substr($line, 6), true);
                        if (isset($data['type']) && $data['type'] === 'content_block_delta') {
                            $callback([
                                'type' => 'token',
                                'content' => $data['delta']['text'],
                            ]);
                        }
                    }
                }
            }

            $callback([
                'type' => 'end',
            ]);
            Log::info('Finished streaming response');
        } catch (RequestException $e) {
            Log::error('Error in streamResponse', [
                'error' => $e->getMessage(),
                'requestBody' => $requestBody ?? null,
                'response' => $e->hasResponse() ? $e->getResponse()->getBody()->getContents() : null
            ]);
            $callback([
                'type' => 'error',
                'content' => 'An error occurred while processing your request: ' . $e->getMessage(),
            ]);
        }
    }
}
