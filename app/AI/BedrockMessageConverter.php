<?php

namespace App\AI;

use Illuminate\Support\Facades\Log;

class BedrockMessageConverter
{
    public function convertToBedrockChatMessages(array $prompt): array
    {
        Log::info('[BedrockMessageConverter] Converting prompt to Bedrock chat messages', [
            'prompt' => json_encode($prompt, JSON_PRETTY_PRINT)
        ]);

        // Check if first non-system message is from user
        $firstNonSystemMessage = null;
        foreach ($prompt as $message) {
            if ($message['role'] !== 'system') {
                $firstNonSystemMessage = $message;
                break;
            }
        }

        if (!$firstNonSystemMessage || $firstNonSystemMessage['role'] !== 'user') {
            throw new \Exception('A conversation must start with a user message (after any system messages).');
        }

        // First pass: collect system messages
        $system = null;
        foreach ($prompt as $message) {
            if ($message['role'] === 'system') {
                if ($system !== null) {
                    throw new \Exception('Multiple system messages are not supported.');
                }
                $system = $message['content'];
            }
        }

        // Second pass: process messages in order
        $messages = [];
        foreach ($prompt as $message) {
            if ($message['role'] === 'system') {
                continue;
            }

            $content = [];
            
            // Handle text content
            if (is_string($message['content'])) {
                $content[] = ['text' => $message['content']];
            } elseif (is_array($message['content'])) {
                foreach ($message['content'] as $part) {
                    if (is_string($part)) {
                        $content[] = ['text' => $part];
                    } elseif (is_array($part) && isset($part['type'])) {
                        switch ($part['type']) {
                            case 'text':
                                $content[] = ['text' => $part['text']];
                                break;
                            case 'tool-call':
                                $content[] = [
                                    'toolUse' => [
                                        'toolUseId' => $part['toolCallId'],
                                        'name' => $part['toolName'],
                                        'input' => $part['args']
                                    ]
                                ];
                                break;
                        }
                    }
                }
            }

            // Handle tool invocations
            if (isset($message['toolInvocations'])) {
                // Add a new user message with the tool results
                $toolResults = [];
                foreach ($message['toolInvocations'] as $toolInvocation) {
                    if ($toolInvocation['state'] === 'result') {
                        $toolResults[] = [
                            'toolResult' => [
                                'toolUseId' => $toolInvocation['toolCallId'],
                                'content' => [['text' => json_encode($toolInvocation['result'])]]
                            ]
                        ];
                    }
                }
                
                if (!empty($toolResults)) {
                    $messages[] = [
                        'role' => 'user',
                        'content' => $toolResults
                    ];
                }
            }

            // Add the original message
            $messages[] = [
                'role' => $message['role'],
                'content' => $content
            ];
        }

        // If the final message is not from user, append a user message saying "Continue."
        if (!empty($messages) && end($messages)['role'] !== 'user') {
            $messages[] = [
                'role' => 'user',
                'content' => [['text' => 'Continue.']]
            ];
        }

        return [
            'system' => $system,
            'messages' => $messages
        ];
    }
}