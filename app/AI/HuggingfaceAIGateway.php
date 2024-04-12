<?php

namespace App\AI;

use Illuminate\Support\Facades\Http;

class HuggingfaceAIGateway
{
    private $apiUrl = 'https://zub38q2qmtrdgl1x.us-east-1.aws.endpoints.huggingface.cloud';

    private $apiKey;

    public function __construct()
    {
        $this->apiKey = config('services.huggingface.api_key');
    }

    public function inference($params)
    {
        $model = $params['model'];
        $messages = $params['messages'];
        $maxTokens = $params['max_tokens'];

        $prompt = $this->formatMessagesToPrompt($messages);

        $data = [
            'inputs' => $prompt,
        ];

        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
            'Authorization' => 'Bearer '.$this->apiKey,
        ])->post($this->apiUrl, $data);

        if ($response->successful()) {
            $result = $response->json();

            $response = $result[0]['generated_text'];

            //            $messages = preg_split('/Human:|Assistant:/', $response, -1, PREG_SPLIT_NO_EMPTY);
            //            $finalResponse = trim(end($messages));

            return [
                'content' => $result,
                'output_tokens' => 0,
                'input_tokens' => 0,
            ];
        } else {
            return [
                'error' => 'Failed to make inference',
                'details' => $response->json(),
            ];
        }
    }

    private function formatMessagesToPrompt($messages)
    {
        $prompt = '';
        foreach ($messages as $message) {
            $role = $message['role'] === 'user' ? 'Human' : 'Assistant';
            $prompt .= "$role: {$message['content']}\n";
        }

        return $prompt;
    }
}
