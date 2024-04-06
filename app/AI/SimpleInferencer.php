<?php

namespace App\AI;

use App\Models\Thread;

class SimpleInferencer
{
    public static function inference(string $prompt, string $model, Thread $thread, callable $streamFunction): string
    {
        $messages = [
            [
                'role' => 'system',
                'content' => 'You are a helpful assistant.',
                // 'content' => 'You are a helpful assistant on OpenAgents.com. Answer the inquiry from the user.',
            ],
            ...get_previous_messages($thread),
            [
                'role' => 'user',
                'content' => $prompt,
            ],
        ];

        $modelDetails = Models::MODELS[$model] ?? null;

        if ($modelDetails) {
            $gateway = $modelDetails['gateway'];
            $maxTokens = $modelDetails['max_tokens'];

            switch ($gateway) {
                case 'anthropic':
                    $client = new AnthropicAIGateway();
                    $inference = $client->createStreamed([
                        'model' => $model,
                        'messages' => $messages,
                        'max_tokens' => $maxTokens,
                        'stream_function' => $streamFunction,
                    ]);
                    break;
                case 'mistral':
                    $client = new MistralAIGateway();
                    $inference = $client->chat()->createStreamed([
                        'model' => $model,
                        'messages' => $messages,
                        'max_tokens' => $maxTokens,
                        'stream_function' => $streamFunction,
                    ]);
                    break;
                case 'openai':
                    $client = new OpenAIGateway();
                    $inference = $client->stream([
                        'model' => $model,
                        'messages' => $messages,
                        'max_tokens' => $maxTokens,
                        'stream_function' => $streamFunction,
                    ]);
                    break;
                    // Add more cases for other gateways if needed
            }
        } else {
            // Handle unknown model
            dd("Unknown model: $model");
        }

        return $inference;
    }
}

function get_previous_messages(Thread $thread)
{
    return $thread->messages()
        ->orderBy('created_at', 'asc')
        ->get()
        ->map(function ($message) {
            // If model is not null, this is agent. Otherwise user
            if ($message->model !== null) {
                $role = 'assistant';
            } else {
                $role = 'user';
            }

            // Check if message content starts with "data:image/" -- but use actual method
            if (strtolower(substr($message->body, 0, 11)) === 'data:image/') {
                $content = '<image>';
            } else {
                $content = $message->body;
            }

            return [
                'role' => $role,
                'content' => $content,
            ];
        })
        ->toArray();
}
