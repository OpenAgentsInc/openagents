<?php

declare(strict_types=1);

namespace App\AI;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;

class PerplexityAIGateway implements GatewayInterface
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

        // Add optional parameters if provided
        if (isset($params['max_tokens'])) {
            $data['max_tokens'] = $params['max_tokens'];
        }
        if (isset($params['temperature'])) {
            $data['temperature'] = $params['temperature'];
        }
        if (isset($params['top_p'])) {
            $data['top_p'] = $params['top_p'];
        }
        if (isset($params['top_k'])) {
            $data['top_k'] = $params['top_k'];
        }
        if (isset($params['presence_penalty'])) {
            $data['presence_penalty'] = $params['presence_penalty'];
        }
        if (isset($params['frequency_penalty'])) {
            $data['frequency_penalty'] = $params['frequency_penalty'];
        }

        try {
            $response = $this->httpClient->request('POST', 'https://api.perplexity.ai/chat/completions', [
                'body' => json_encode($data),
                'headers' => [
                    'accept' => 'application/json',
                    'Authorization' => 'Bearer '.env('PERPLEXITY_API_KEY'),
                    'content-type' => 'application/json',
                ],
            ]);

            return $this->extractData($response, $data['stream'], $params['stream_function']);

        } catch (RequestException $e) {
            dd($e->getMessage());
        }
    }
}
