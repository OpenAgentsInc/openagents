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

            $buffer = ''; // Accumulate partial data that hasn't yet formed a complete JSON object

            while (! $stream->eof()) {
                $buffer .= $stream->read(1024);
                while (($newlinePos = strpos($buffer, "\n")) !== false) {
                    $line = substr($buffer, 0, $newlinePos);
                    $buffer = substr($buffer, $newlinePos + 1); // Remove the processed line from the buffer

                    $data = json_decode($line, true); // Decode the line as JSON
                    if ($data) {
                        dd($data);
                        // Process the decoded data
                        if ($data['type'] === 'content_block_delta' && isset($data['delta']['text'])) {
                            $content .= $data['delta']['text']; // Concatenate text deltas to form the complete message
                        }
                        // Handle other event types as needed...
                    }
                }
            }

            //            while (! $stream->eof()) {
            //                $line = $stream->read(1024); // Adjust the chunk size as needed
            //                dd($line);
            //                $data = json_decode($line, true);
            //                if ($data) {
            //                    $content .= $data['text'] ?? ''; // Append the content or handle it as per your requirement
            //                    call_user_func($params['stream_function'], $data);
            //                }
            //            }

            return $content;
        } catch (RequestException $e) {
            // Handle exception or error
            dd($e->getMessage());

            return 'Error: '.$e->getMessage();
        }
    }
}
