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
        // dont care about params model cuz for now we only have one model, deployed at the api url
        $messages = $params['messages'];

        $prompt = $this->formatMessagesToPrompt($messages);

        $data = [
            'inputs' => $prompt,
            'parameters' => [
                'return_full_text' => false,
                'max_length' => 200,
                'max_new_tokens' => 200,
            ],
        ];

        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
            'Authorization' => 'Bearer '.$this->apiKey,
        ])->post($this->apiUrl, $data);

        if ($response->successful()) {
            $result = $response->json();

            $response = $result[0]['generated_text'];
            // Explode response by "Assistant: " and take the second thing
            $actualresponse = explode('Assistant: ', $response);

            // Check if the $actualresponse array has at least two elements
            if (count($actualresponse) > 1) {
                $finalResponse = $actualresponse[1];

                // If this response says "Human:" anywhere, then explode it and take everything else
                if (strpos($finalResponse, 'Human:') !== false) {
                    $actualresponse = explode('Human: ', $finalResponse);
                    $finalResponse = $actualresponse[0];
                }
            } else {
                // If the "Assistant: " pattern is not found, use the entire response
                $finalResponse = $response;
            }

            return [
                'content' => $finalResponse,
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

    public function formatMessagesForConversation($messages)
    {
        return [];
        $pastUserInputs = [];
        $generatedResponses = [];

        foreach ($messages as $message) {
            if ($message['role'] === 'user') {
                $pastUserInputs[] = $message['content'];
            } else {
                $generatedResponses[] = $message['content'];
            }
        }

        return [
            'past_user_inputs' => $pastUserInputs,
            'generated_responses' => $generatedResponses,
        ];
    }
}
