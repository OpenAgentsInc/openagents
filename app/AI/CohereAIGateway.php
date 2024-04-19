<?php

declare(strict_types=1);

namespace App\AI;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;

class CohereAIGateway implements GatewayInterface
{
    private Client $httpClient;

    public function __construct(Client $httpClient)
    {
        $this->httpClient = $httpClient;
    }

    public function summarize(string $conversationText): ?string
    {
        $data = [
            'message' => "What is a 3- to 5-word phrase that summarizes the following conversation? Capitalize the first letter of each word. Respond only with the phrase, no other words.\n\n$conversationText",
            'model' => 'command-r',
            'temperature' => 0,
            'max_tokens' => 10,
        ];

        try {
            $response = $this->httpClient->post('https://api.cohere.ai/v1/chat', [
                'json' => $data,
                'headers' => [
                    'Content-Type' => 'application/json',
                    'Authorization' => 'Bearer '.env('COHERE_API_KEY'),
                    'accept' => 'application/json',
                ],
            ]);

            $responseData = json_decode($response->getBody()->getContents(), true);

            return $responseData['text'];
        } catch (RequestException $e) {
            dd($e->getMessage());
        }
    }

    public function inference(array $params): array
    {
        $data = [
            'message' => $params['message'],
            'model' => $params['model'] ?? 'command-r',
            'stream' => $params['stream'] ?? false,
            'preamble' => $params['preamble'] ?? null,
            'chat_history' => [],
            'conversation_id' => $params['conversation_id'] ?? null,
            'prompt_truncation' => $params['prompt_truncation'] ?? 'AUTO',
            'connectors' => $params['connectors'] ?? [],
        ];

        if (isset($params['chat_history'])) {
            foreach ($params['chat_history'] as $message) {
                $data['chat_history'][] = [
                    'role' => $message['role'],
                    'message' => $message['content'],
                ];
            }
        }

        try {
            $response = $this->httpClient->post('https://api.cohere.ai/v1/chat', [
                'json' => $data,
                'headers' => [
                    'Content-Type' => 'application/json',
                    'Authorization' => 'Bearer '.env('COHERE_API_KEY'),
                    'accept' => 'application/json',
                ],
            ]);

            $responseData = json_decode($response->getBody()->getContents(), true);

            return [
                'content' => $responseData['text'],
                'output_tokens' => $responseData['meta']['tokens']['output_tokens'],
                'input_tokens' => $responseData['meta']['tokens']['input_tokens'],
            ];
        } catch (RequestException $e) {
            dd($e->getMessage());
        }
    }
}
