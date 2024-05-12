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

            if ($data['stream']) {
                return $this->extractFromStream($response, $params['stream_function']);
            }
            $responseData = json_decode($response->getBody()->getContents(), true);

            return [
                'content' => $responseData['choices'][0]['message']['content'] ?? '',
                'output_tokens' => $responseData['usage']['completion_tokens'] ?? 0,
                'input_tokens' => $responseData['usage']['prompt_tokens'] ?? 0,
            ];
        } catch (RequestException $e) {
            dd($e->getMessage());
        }
    }
}
