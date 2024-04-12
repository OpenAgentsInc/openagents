<?php

namespace App\AI;

use Illuminate\Support\Facades\Http;

class HuggingfaceAIGateway
{
    private $apiUrl = 'https://api-inference.huggingface.co/models/microsoft/DialoGPT-large';
    //    private $apiUrl = 'https://zub38q2qmtrdgl1x.us-east-1.aws.endpoints.huggingface.cloud';

    private $apiKey;

    public function __construct()
    {
        $this->apiKey = config('services.huggingface.api_key');
    }

    public function conversationalInference($params)
    {
        $inputs = [
            'text' => $params['text'],
            'generated_responses' => $params['generated_responses'] ?? [],
            'past_user_inputs' => $params['past_user_inputs'] ?? [],
        ];

        $options = $params['options'] ?? [
            'use_cache' => true,
            'wait_for_model' => false,
        ];

        $parameters = $params['parameters'] ?? [
            'min_length' => null,
            'max_length' => 1000,
            'top_k' => null,
            'top_p' => null,
            'temperature' => 1.0,
            'repetition_penalty' => null,
            'max_time' => null,
        ];

        $data = [
            'inputs' => json_encode($inputs),
            'options' => $options,
            'parameters' => $parameters,
        ];

        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
            'Authorization' => 'Bearer '.$this->apiKey,
        ])->post($this->apiUrl, $data);

        dd($response->json());

        if ($response->successful()) {
            $result = $response->json();

            return $result;
        } else {
            return [
                'error' => 'Failed to make conversational inference',
                'details' => $response->json(),
            ];
        }
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
