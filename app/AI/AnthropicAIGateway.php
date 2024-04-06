<?php

namespace App\AI;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;

class AnthropicAIGateway
{
    private $client;

    private $apiBaseUrl = 'https://api.anthropic.com';

    public function __construct()
    {
        $this->client = new Client([
            // Base URI is used with relative requests
            'base_uri' => $this->apiBaseUrl,
            // You can set any number of default request options.
            'headers' => [
                'Content-Type' => 'application/json',
                'x-api-key' => env('ANTHROPIC_API_KEY'), // Make sure to set your API key in your .env file
                'anthropic-version' => '2023-06-01',
                'anthropic-beta' => 'messages-2023-12-15',
            ],
        ]);
    }

    public function createStreamed($params)
    {
        $client = new Client();

        // If the role of the first message is 'system', remove that and set it as a separate variable
        $systemMessage = null;
        if ($params['messages'][0]['role'] === 'system') {
            $systemMessage = array_shift($params['messages'])['content'];
        }

        array_pop($params['messages']);

        $data = [
            'model' => $params['model'],
            'messages' => $params['messages'],
            'max_tokens' => $params['max_tokens'],
            'stream' => true, // Ensure this is true for streaming
            'system' => $systemMessage ?? 'You are a helpful assistant.',
        ];

        // Add optional parameters if provided
        if (isset($params['temperature'])) {
            $data['temperature'] = $params['temperature'];
        }
        if (isset($params['top_p'])) {
            $data['top_p'] = $params['top_p'];
        }

        try {
            $response = $client->post('https://api.anthropic.com/v1/messages', [
                'json' => $data,
                'stream' => true, // Important for handling streaming responses
                'headers' => [
                    'Content-Type' => 'application/json',
                    'x-api-key' => env('ANTHROPIC_API_KEY'), // Make sure to set your API key in your .env file
                    'anthropic-version' => '2023-06-01',
                    'anthropic-beta' => 'messages-2023-12-15',
                ],
            ]);

            $stream = $response->getBody();

            $content = '';
            foreach ($this->readStream($stream) as $event) {
                if ($event['type'] === 'content_block_delta' && isset($event['delta']['text'])) {
                    $content .= $event['delta']['text'];
                }
                // Process other event types as needed
            }

            return $content;
        } catch (RequestException $e) {
            // Handle exception or error
            dd($e->getMessage());

            return 'Error: '.$e->getMessage();
        }
    }

    private function readStream($stream)
    {
        $buffer = '';
        while (! $stream->eof()) {
            $buffer .= $stream->read(1024);
            while (($pos = strpos($buffer, "\n")) !== false) {
                $line = substr($buffer, 0, $pos);
                $buffer = substr($buffer, $pos + 1);

                if (str_starts_with($line, 'data: ')) {
                    $data = json_decode(trim(substr($line, 5)), true);
                    if ($data) {
                        yield $data;
                    }
                }
            }
        }
    }
}
