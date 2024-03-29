<?php

namespace App\AI;

use Illuminate\Support\Facades\Http;

class GeminiAIGateway
{
    protected $apiKey;

    protected $baseUrl = 'https://generativelanguage.googleapis.com';

    protected $defaultModel = 'gemini-pro'; // Default text-only model

    protected $newModel = 'gemini-1.5-pro-latest';

    protected $visionModel = 'gemini-pro-vision'; // Model for text and image prompts

    public function __construct()
    {
        $this->apiKey = env('GEMINI_API_KEY');
    }

    public function inference(string|array $prompt, ?string $model = null): array
    {
        // Determine the model to use based on prompt type and optional parameter
        if (is_array($prompt) && array_key_exists('contents', $prompt)) {
            // Assume prompts with 'contents' key contain image data, use vision model
            $modelPath = $this->visionModel;
        } else {
            // Use default text-only model or specified model
            $modelPath = $model === 'new' ? $this->newModel : $this->defaultModel;
        }

        $url = "{$this->baseUrl}/v1beta/models/{$modelPath}:generateContent?key={$this->apiKey}";

        $blob = [
            'contents' => [
                [
                    'parts' => [
                        ['text' => $prompt],
                    ],
                ],
            ],
        ];

        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
        ])->post($url, $blob);

        dump($response->json());

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
