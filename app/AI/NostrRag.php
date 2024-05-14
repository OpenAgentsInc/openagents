<?php

declare(strict_types=1);

namespace App\AI;

use App\Models\Thread;
use OpenAI;

class NostrRag
{
    protected $messages;

    protected $agent_id;

    protected $prompt;

    public function history(Thread $thread, int $maxTokens = 2000)
    {
        $messages = [];
        $tokenCount = 0;
        $userContent = '';

        foreach ($thread->messages()->orderBy('created_at', 'asc')->get() as $message) {
            if ($message->model !== null) {
                $role = 'assistant';
            } else {
                $role = 'user';
            }

            if ($role === 'user') {
                if (strtolower(substr($message->body, 0, 11)) === 'data:image/') {
                    $userContent .= ' <image>';
                } else {
                    $userContent .= ' '.$message->body;
                }
            } else {
                if (! empty($userContent)) {
                    $messageTokens = ceil(str_word_count($userContent) / 3);

                    if ($tokenCount + $messageTokens > $maxTokens) {
                        break; // Stop adding messages if the remaining context is not enough
                    }

                    $messages[] = [
                        'role' => 'user',
                        'content' => trim($userContent),
                    ];

                    $tokenCount += $messageTokens;
                    $userContent = '';
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
                    'role' => 'assistant',
                    'content' => $content,
                ];

                $tokenCount += $messageTokens;
            }
        }

        if (! empty($userContent)) {
            $messageTokens = ceil(str_word_count($userContent) / 3);

            if ($tokenCount + $messageTokens <= $maxTokens) {
                $messages[] = [
                    'role' => 'user',
                    'content' => trim($userContent),
                ];
            }
        }

        $this->messages = $messages;

        return $this;
    }

    public function send()
    {

        $ApiKey = config('services.openai.api_key');
        $response = OpenAI::client($ApiKey)->chat()->create([
            'model' => 'gpt-3.5-turbo',
            'messages' => [
                ['role' => 'system', 'content' => $this->prompt],
            ],
            'max_tokens' => 2048,
            'temperature' => 0.5,
        ]);

        return $response->choices[0]->message->content ?? '';

    }

    public function summary()
    {

        // Convert the messages array into a string
        $chatHistory = implode("\n", array_map(function ($message) {
            return $message['role'].': '.$message['content'];
        }, $this->messages));

        // Construct the prompt with the chat history
        $content = "Given the following chat history between user and assistant,
        answer with a fully qualified standalone and short question that summarizes the user's question.

        CHAT HISTORY:
        $chatHistory

        FULLY QUALIFIED QUESTION: ";

        $this->prompt = $content;

        return $this->send();

    }
}
