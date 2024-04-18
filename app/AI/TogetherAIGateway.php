<?php

declare(strict_types=1);

namespace App\AI;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;

class TogetherAIGateway implements GatewayInterface
{
    private Client $httpClient;

    public function __construct(Client $httpClient)
    {
        $this->httpClient = $httpClient;
    }

    public function inference(array $params): array
    {
        $data = [
            'model' => $params['model'],
            'messages' => [],
        ];

        if (isset($params['chat_history'])) {
            foreach ($params['chat_history'] as $message) {
                $data['messages'][] = [
                    'role' => $message['role'],
                    'content' => $message['content'],
                ];
            }
        }

        try {
            $response = $this->httpClient->post('https://api.together.xyz/v1/chat/completions', [
                'json' => $data,
                'headers' => [
                    'Content-Type' => 'application/json',
                    'Authorization' => 'Bearer '.env('TOGETHER_API_KEY'),
                ],
            ]);

            $responseData = json_decode($response->getBody()->getContents(), true);

            return [
                'content' => $responseData['choices'][0]['message']['content'],
                'output_tokens' => $responseData['usage']['completion_tokens'],
                'input_tokens' => $responseData['usage']['prompt_tokens'],
            ];
        } catch (RequestException $e) {
            dd($e->getMessage());
        }
    }
}
