<?php

namespace App\AI;

use App\Traits\StreamingTrait;
use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;
use Illuminate\Support\Facades\Log;

class GroqAIGateway
{
    use StreamingTrait;

    private Client $httpClient;

    public function __construct(?Client $httpClient = null)
    {
        $this->httpClient = $httpClient ?? new Client();
    }

    public function inference(array $params)
    {
        $apiKey = env('GROQ_API_KEY');
        if (empty($apiKey)) {
            throw new \RuntimeException('GROQ_API_KEY is not set in the environment');
        }

        try {
            $params = $this->convertToolInvocationsToToolCalls($params);

            Log::info('GroqAIGateway request', [
                'url' => 'https://api.groq.com/openai/v1/chat/completions',
                'params' => $params,
                'headers' => [
                    'Content-Type' => 'application/json',
                    'Authorization' => 'Bearer ' . substr($apiKey, 0, 5) . '...',
                ],
            ]);

            $response = $this->httpClient->post('https://api.groq.com/openai/v1/chat/completions', [
                'json' => $params,
                'headers' => [
                    'Content-Type' => 'application/json',
                    'Authorization' => 'Bearer ' . $apiKey,
                ],
            ]);

            $contentType = $response->getHeaderLine('Content-Type');
            if (strpos($contentType, 'application/json') === false) {
                Log::warning('Unexpected content type from Groq API', ['Content-Type' => $contentType]);
            }

            $responseBody = (string) $response->getBody();
            Log::info('GroqAIGateway response', [
                'status' => $response->getStatusCode(),
                'headers' => $response->getHeaders(),
                'body' => $responseBody,
            ]);

            return $this->extractData($response, $responseBody, $params['stream'] ?? false, $params['stream_function'] ?? null);
        } catch (RequestException $e) {
            Log::error('GroqAIGateway error', [
                'error' => $e->getMessage(),
                'request' => $e->getRequest(),
                'response' => $e->getResponse() ? [
                    'status' => $e->getResponse()->getStatusCode(),
                    'body' => (string) $e->getResponse()->getBody(),
                ] : null,
            ]);
            throw new \RuntimeException('Error calling Groq API: ' . $e->getMessage(), 0, $e);
        }
    }

    private function convertToolInvocationsToToolCalls(array $params): array
    {
        if (!isset($params['messages'])) {
            return $params;
        }

        foreach ($params['messages'] as &$message) {
            if ($message['role'] === 'assistant' && isset($message['toolInvocations'])) {
                $message['tool_calls'] = array_map(function ($invocation) {
                    return [
                        'id' => $invocation['toolCallId'],
                        'type' => 'function',
                        'function' => [
                            'name' => $invocation['toolName'],
                            'arguments' => json_encode($invocation['args'])
                        ]
                    ];
                }, $message['toolInvocations']);

                unset($message['toolInvocations']);
            }
        }

        return $params;
    }
}
