<?php

declare(strict_types=1);

namespace App\AI;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;

class NewGeminiAIGateway implements GatewayInterface
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
            'contents' => [],
            'generationConfig' => [],
        ];
        foreach ($params['messages'] as $message) {
            $data['contents'][] = [
                'role' => $message['role'] === 'model' ? 'model' : 'user',
                'parts' => [
                    'text' => $message['content'],
                ],
            ];
        }
        $stream = $params['stream'] ?? true;
        // Add optional parameters if provided
        if (isset($params['max_tokens'])) {
            $data['generationConfig']['maxOutputTokens'] = $params['max_tokens'];
        }
        if (isset($params['temperature'])) {
            $data['generationConfig']['temperature'] = $params['temperature'];
        }
        if (isset($params['top_p'])) {
            $data['generationConfig']['topP'] = $params['top_p'];
        }
        if (isset($params['top_k'])) {
            $data['generationConfig']['topK'] = $params['top_k'];
        }

        try {
            $url = 'https://generativelanguage.googleapis.com/v1beta/models/'
                .$params['model'].':';
            $url .= $stream ? 'streamGenerateContent?alt=sse&' : 'generateContent?';
            $response = $this->httpClient->request(
                'POST',
                $url.'key='.env('GEMINI_API_KEY'),
                [
                    'body' => json_encode($data),
                    'stream' => $stream,
                    'headers' => [
                        'content-type' => 'application/json',
                    ],
                ]
            );

            return $this->extractData($response, $stream, $params['stream_function']);

        } catch (RequestException $e) {
            dd($e->getMessage());
        }
    }

    protected function extractFromJson(array $responseData): array
    {
        return [
            'content' => $responseData['candidates'][0]['content']['parts'][0]['text'] ?? '',
            'output_tokens' => $responseData['usageMetadata']['candidatesTokenCount'] ?? 0,
            'input_tokens' => $responseData['usageMetadata']['promptTokenCount'] ?? 0,
        ];
    }

    protected function extractTokens(array $event, callable $streamFunction)
    {
        if (isset($event['candidates'][0]['content']['parts'])) {
            $text = $event['candidates'][0]['content']['parts'][0]['text'] ?? '';
            $this->data['content'] .= $text;
            $streamFunction($text);
        }
        if (isset($event['usageMetadata'])) {
            $this->data['input_tokens'] = $event['usageMetadata']['promptTokenCount'];
            $this->data['output_tokens'] = $event['usageMetadata']['candidatesTokenCount'];
        }
    }
}
