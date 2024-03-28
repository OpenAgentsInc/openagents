<?php

namespace App\AI;

use Illuminate\Support\Facades\Http;

class GeminiAIGateway
{
    protected $apiKey;

    protected $baseUrl = 'https://generativelanguage.googleapis.com';

    public function __construct()
    {
        $this->apiKey = env('GEMINI_API_KEY');
    }

    public function inference(string $text): array
    {
        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
        ])->post("{$this->baseUrl}/v1/models/gemini-pro:generateContent?key={$this->apiKey}", [
            'contents' => [
                [
                    'parts' => [
                        ['text' => $text],
                    ],
                ],
            ],
        ]);

        if ($response->successful()) {
            return $response->json();
        } else {
            return [];
        }
    }

    public function chat(array $messages): array
    {
        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
        ])->post("{$this->baseUrl}/v1beta/models/gemini-pro:generateContent?key={$this->apiKey}", [
            'contents' => array_map(function ($message) {
                return [
                    'role' => $message['role'],
                    'parts' => [
                        ['text' => $message['text']],
                    ],
                ];
            }, $messages),
        ]);

        if ($response->successful()) {
            return $response->json();
        } else {
            return [];
        }
    }
}
