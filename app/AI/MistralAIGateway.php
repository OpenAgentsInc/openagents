<?php

namespace App\AI;

use Illuminate\Support\Facades\Http;

class MistralAIGateway {
    public function inference($messages) {
        // Your API endpoint
        $url = 'https://api.mistral.ai/v1/chat/completions';

        // Prepare the data payload
        $data = [
            'model' => 'mistral-large-latest',
            'messages' => $messages,
            'max_tokens' => 2000,
            'temperature' => 0.5,
            'top_p' => 1,
        ];

        // Make the HTTP POST request
        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
            'Authorization' => 'Bearer ' . env('MISTRAL_API_KEY'),
        ])->post($url, $data);

        // Check if the request was successful
        if ($response->successful()) {
            // Return the response body
            return $response->json();
        } else {
            // Handle the error (you can customize this part based on your needs)
            return [
                'error' => 'Failed to make inference',
                'details' => $response->json(),
            ];
        }
    }
}
