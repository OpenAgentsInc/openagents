<?php

declare(strict_types=1);

namespace App\AI;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;

class AnthropicAIGateway implements GatewayInterface
{
    use StreamingTrait;

    private Client $httpClient;

    public function __construct(Client $httpClient)
    {
        $this->httpClient = $httpClient;
    }

    public function inference(array $params): array
    {
        // If the role of the first message is 'system', remove that and set it as a separate variable
        $systemMessage = null;
        if ($params['messages'][0]['role'] === 'system') {
            $systemMessage = array_shift($params['messages'])['content'];
        }

        $data = [
            'model' => $params['model'],
            'messages' => $params['messages'],
            'max_tokens' => $params['max_tokens'],
            'stream' => $params['stream'] ?? true,
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
            $response = $this->httpClient->post('https://api.anthropic.com/v1/messages', [
                'json' => $data,
                'stream' => true, // Important for handling streaming responses
                'headers' => [
                    'Content-Type' => 'application/json',
                    'x-api-key' => env('ANTHROPIC_API_KEY'), // Make sure to set your API key in your .env file
                    'anthropic-version' => '2023-06-01',
                    'anthropic-beta' => 'messages-2023-12-15',
                ],
            ]);
            if ($data['stream']) {
                return $this->extractFromStream($response, $params['stream_function']);
            }
            $responseData = json_decode($response->getBody()->getContents(), true);

            return [
                'content' => $responseData['content'][0]['text'],
                'output_tokens' => $responseData['usage']['output_tokens'],
                'input_tokens' => $responseData['usage']['input_tokens'],
            ];
        } catch (RequestException $e) {
            dd($e->getMessage());
        }
    }

    // Overriden from StreamingTrait
    protected function extractTokens(array $event, callable $streamFunction)
    {
        if ($event['type'] === 'content_block_delta' && isset($event['delta']['text'])) {
            $this->data['content'] .= $event['delta']['text'];
            $streamFunction($event['delta']['text']);
        } elseif ($event['type'] === 'message_start' && isset($event['message']['usage']['input_tokens'])) {
            $this->data['input_tokens'] = $event['message']['usage']['input_tokens'];
        } elseif ($event['type'] === 'message_delta' && isset($event['usage']['output_tokens'])) {
            $this->data['output_tokens'] = $event['usage']['output_tokens'];
        }
    }
}
