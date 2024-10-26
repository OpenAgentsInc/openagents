<?php

namespace App\Services;

use App\Models\Thread;

class ChatService
{
    private $chat;

    // constructor taking chat id
    public function __construct(int $chatId)
    {
        $this->chat = Thread::findOrFail($chatId);
    }

    public function getRelevantMessages()
    {
        // First retrieve all messages from this chat
        $messages = $this->chat->chatMessages;

        // Then format them in role/content. Role user if message has a user_id, assistant otherwise
        $formattedMessages = $messages->map(function ($message) {
            return [
                'role' => $message->user_id ? 'user' : 'assistant',
                'content' => $message->content
            ];
        });

        // Filter out messages with empty, null, or blank content
        $filteredMessages = $formattedMessages->filter(function ($message) {
            return !empty(trim($message['content']));
        })->values();

        // If any messages have user as role multiple consecutive times, merge them into one message
        $mergedMessages = $this->mergeConsecutiveMessages($filteredMessages);

        return $mergedMessages;
    }

    private function mergeConsecutiveMessages($messages)
    {
        $mergedMessages = [];
        $currentRole = null;
        $currentContent = '';

        foreach ($messages as $message) {
            if ($message['role'] === $currentRole) {
                $currentContent .= "\n\n" . $message['content'];
            } else {
                if ($currentRole !== null) {
                    $mergedMessages[] = [
                        'role' => $currentRole,
                        'content' => $currentContent
                    ];
                }
                $currentRole = $message['role'];
                $currentContent = $message['content'];
            }
        }

        if ($currentRole !== null) {
            $mergedMessages[] = [
                'role' => $currentRole,
                'content' => $currentContent
            ];
        }

        return $mergedMessages;
    }
}
