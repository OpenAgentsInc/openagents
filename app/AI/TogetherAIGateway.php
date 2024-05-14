<?php

declare(strict_types=1);

namespace App\AI;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;

class TogetherAIGateway implements GatewayInterface
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
            'stream' => $params['stream'] ?? true,
            'messages' => array_map(function ($message) {
                return [
                    'role' => $message['role'],
                    'content' => $message['content'],
                ];
            }, $params['messages']),
        ];

        try {
            $response = $this->httpClient->post('https://api.together.xyz/v1/chat/completions', [
                'json' => $data,
                'headers' => [
                    'Content-Type' => 'application/json',
                    'Authorization' => 'Bearer '.env('TOGETHER_API_KEY'),
                ],
            ]);

            return $this->extractData($response, $data['stream'], $params['stream_function']);

        } catch (RequestException $e) {
            dd($e->getMessage());
        }
    }
}
