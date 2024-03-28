<?php

namespace App\AI;

use Illuminate\Support\Facades\Http;

class GeminiAIGateway
{
    protected $apiKey;

    protected $baseUrl = 'https://generativelanguage.googleapis.com';

    protected $defaultModel = 'gemini-pro'; // Default model

    protected $newModel = 'gemini-1.5-pro-latest'; // New model

    public function __construct()
    {
        $this->apiKey = env('GEMINI_API_KEY');
    }

    public function inference(string $text, ?string $model = null): array
    {
        $isNewModel = $model === 'new';
        $modelPath = $isNewModel ? $this->newModel : $this->defaultModel;
        $apiVersion = $isNewModel ? 'v1beta' : 'v1';

        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
        ])->post("{$this->baseUrl}/{$apiVersion}/models/{$modelPath}:generateContent?key={$this->apiKey}", [
            'contents' => [
                [
                    'parts' => [
                        ['text' => $text],
                    ],
                ],
            ],
        ]);

        return $response->successful() ? $response->json() : [
            'error' => 'Failed to generate inference',
            'details' => $response->json(),
        ];
    }

    public function chat(array $messages, ?string $model = null): array
    {
        $modelPath = $model === 'new' ? $this->newModel : $this->defaultModel;

        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
        ])->post("{$this->baseUrl}/v1beta/models/{$modelPath}:generateContent?key={$this->apiKey}", [
            'contents' => array_map(function ($message) {
                return [
                    'role' => $message['role'],
                    'parts' => [
                        ['text' => $message['text']],
                    ],
                ];
            }, $messages),
        ]);

        return $response->successful() ? $response->json() : [];
    }
}
