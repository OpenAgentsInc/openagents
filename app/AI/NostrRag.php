<?php

declare(strict_types=1);

namespace App\AI;

use App\Models\Thread;
use App\Services\OpenObserveLogger;
use OpenAI;

class NostrRag
{
    protected array $messages;

    protected string $prompt;

    public function history(Thread $thread, int $maxTokens = 14000): NostrRag
    {
        $inference = new SimpleInferencer();
        $this->messages = $inference->getTruncatedMessages($thread, $maxTokens);

        return $this;
    }

    public function getMessages(): array
    {
        return $this->messages;
    }

    public function send()
    {
        $apiKey = config('services.openai.api_key');
        $response = OpenAI::client($apiKey)->chat()->create([
            'model' => 'gpt-3.5-turbo-16k',
            'messages' => [
                ['role' => 'system', 'content' => $this->prompt],
            ],
            'temperature' => 0.5,
        ]);

        return $response->choices[0]->message->content ?? '';
    }

    public function summary()
    {
        // Convert the messages array into a string
        $chatHistory = '';
        for ($i = 0; $i < count($this->messages); $i++) {
            $message = $this->messages[$i];
            $role = $message['role'];
            $content = $message['content'];
            if ($role == 'system') {
                continue;
            } // Skip system messages
            $chatHistory .= "$role: $content\n";
        }
        // Construct the prompt with the chat history
        $content = "\
Given the following chat history between user and assistant, answer with a fully qualified short standalone question to retrieve more context.

CHAT HISTORY:
$chatHistory

FULLY QUALIFIED QUESTION: ";

        $this->prompt = $content;

        $logger = new OpenObserveLogger([
        ]);
        $logger->log('info', 'Using RAG prompt '.$content);
        $logger->close();

        return $this->send();
    }
}
