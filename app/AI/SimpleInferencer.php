<?php

namespace App\AI;

use App\Models\Thread;

class SimpleInferencer
{
    public static function inference(string $prompt, string $model, Thread $thread, callable $streamFunction): string
    {
        $modelDetails = Models::MODELS[$model] ?? null;

        if ($modelDetails) {
            $gateway = $modelDetails['gateway'];
            $maxTokens = $modelDetails['max_tokens'];

            $messages = [
                [
                    'role' => 'system',
                    'content' => 'You are a helpful assistant.',
                    // 'content' => 'You are a helpful assistant on OpenAgents.com. Answer the inquiry from the user.',
                ],
                ...get_truncated_messages($thread, $maxTokens),
                [
                    'role' => 'user',
                    'content' => $prompt,
                ],
            ];

            // Calculate the approximate number of tokens in the messages
            $messageTokens = array_sum(array_map(function ($message) {
                return ceil(str_word_count($message['content']) / 3);
            }, $messages));

            // Adjust the max_tokens value for the completion
            $completionTokens = $maxTokens - $messageTokens;

            switch ($gateway) {
                case 'anthropic':
                    $client = new AnthropicAIGateway();
                    $inference = $client->createStreamed([
                        'model' => $model,
                        'messages' => $messages,
                        'max_tokens' => $completionTokens,
                        'stream_function' => $streamFunction,
                    ]);
                    break;
                case 'mistral':
                    $client = new MistralAIGateway();
                    $inference = $client->chat()->createStreamed([
                        'model' => $model,
                        'messages' => $messages,
                        'max_tokens' => $completionTokens,
                        'stream_function' => $streamFunction,
                    ]);
                    break;
                case 'openai':
                    $client = new OpenAIGateway();
                    $inference = $client->stream([
                        'model' => $model,
                        'messages' => $messages,
                        'max_tokens' => $completionTokens,
                        'stream_function' => $streamFunction,
                    ]);
                    break;
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
    $messages = [];
    $previous_role = null;

    foreach ($thread->messages()->orderBy('created_at', 'asc')->get() as $message) {
        // If model is not null, this is agent. Otherwise user
        $role = $message->model !== null ? 'assistant' : 'user';

        // Check if message content starts with "data:image/" -- but use actual method
        $content = strtolower(substr($message->body, 0, 11)) === 'data:image/' ? '<image>' : $message->body;

        // If the role is different from the previous role, add the message
        if ($role !== $previous_role) {
            $messages[] = [
                'role' => $role,
                'content' => $content,
            ];
            $previous_role = $role;
        }
    }

    return $messages;
}

function get_truncated_messages(Thread $thread, int $maxTokens)
{
    $messages = [];
    $tokenCount = 0;

    foreach ($thread->messages()->orderBy('created_at', 'asc')->get() as $message) {
        if ($message->model !== null) {
            $role = 'assistant';
        } else {
            $role = 'user';
        }

        if (strtolower(substr($message->body, 0, 11)) === 'data:image/') {
            $content = '<image>';
        } else {
            $content = $message->body;
        }

        $messageTokens = ceil(str_word_count($content) / 3);

        if ($tokenCount + $messageTokens > $maxTokens) {
            break; // Stop adding messages if the remaining context is not enough
        }

        $messages[] = [
            'role' => $role,
            'content' => $content,
        ];

        $tokenCount += $messageTokens;
    }

    return $messages;
}
