<?php

declare(strict_types=1);

namespace App\AI;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;

class GroqAIGateway implements GatewayInterface
{
    use StreamingTrait;

    private Client $httpClient;

    public function __construct(Client $httpClient)
    {
        $this->httpClient = $httpClient;
    }

    public function inference(array $params): array
    {
        $data = [
            'model' => $params['model'],
            'messages' => $params['messages'],
            'stream' => $params['stream'] ?? true,
        ];
        if (isset($params['max_tokens'])) {
            $data['max_tokens'] = $params['max_tokens'];
        }

        try {
            $response = $this->httpClient->post('https://api.groq.com/openai/v1/chat/completions', [
                'json' => $data,
                'headers' => [
                    'Content-Type' => 'application/json',
                    'Authorization' => 'Bearer '.env('GROQ_API_KEY'),
                ],
            ]);

            return $this->extractData($response, $data['stream'], $params['stream_function']);

        } catch (RequestException $e) {
            dd($e->getMessage());
        }
    }
}
