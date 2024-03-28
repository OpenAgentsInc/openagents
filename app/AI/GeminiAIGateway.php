<?php

namespace App\AI;

use Illuminate\Support\Facades\Http;

class GeminiAIGateway
{
    public function inference(string $text): array
    {
        $apiKey = env('GEMINI_API_KEY');
        $baseUrl = 'https://generativelanguage.googleapis.com';

        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
        ])->post("{$baseUrl}/v1/models/gemini-pro:generateContent?key={$apiKey}", [
            'contents' => [
                [
                    'parts' => [
                        ['text' => $text],
                    ],
                ],
            ],
        ]);

        // Check if the request was successful
        if ($response->successful()) {
            return $response->json();
        } else {
            // Log error details or handle the error
            // For simplicity, we're returning an empty array to indicate failure
            return [];
        }
    }
}
