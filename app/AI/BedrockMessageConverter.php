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

        // First pass: collect system messages and validate message sequence
        $system = null;
        $lastRole = null;
        foreach ($prompt as $message) {
            if ($message['role'] === 'system') {
                if ($system !== null) {
                    throw new \Exception('Multiple system messages are not supported.');
                }
                $system = $this->formatContent($message['content']);
                continue;
            }

            if ($message['role'] === 'assistant' && $lastRole === 'assistant') {
                throw new \Exception('Consecutive assistant messages are not allowed.');
            }
            $lastRole = $message['role'];
        }

        // Second pass: process messages in order
        $messages = [];
        $lastRole = null;
        $pendingToolUse = null;

        foreach ($prompt as $message) {
            if ($message['role'] === 'system') {
                continue;
            }

            // Add the message
            if ($message['role'] === 'user' && $lastRole === 'user') {
                // Insert an assistant message to maintain alternation
                $messages[] = [
                    'role' => 'assistant',
                    'content' => [['text' => 'I understand.']]
                ];
            }

            $formattedContent = $this->formatContent($message['content']);
            $messages[] = [
                'role' => $message['role'],
                'content' => $formattedContent
            ];
            $lastRole = $message['role'];

            // Track tool use for later matching with results
            if ($message['role'] === 'assistant') {
                foreach ($formattedContent as $block) {
                    if (isset($block['toolUse'])) {
                        $pendingToolUse = $block['toolUse']['toolUseId'];
                    }
                }
            }

            // Handle tool invocations immediately after the assistant message
            if (isset($message['toolInvocations'])) {
                foreach ($message['toolInvocations'] as $toolInvocation) {
                    if ($toolInvocation['state'] === 'result' && $pendingToolUse === $toolInvocation['toolCallId']) {
                        $messages[] = [
                            'role' => 'user',
                            'content' => [
                                [
                                    'toolResult' => [
                                        'toolUseId' => $toolInvocation['toolCallId'],
                                        'content' => [['text' => json_encode($toolInvocation['result'])]]
                                    ]
                                ]
                            ]
                        ];
                        $lastRole = 'user';
                        $pendingToolUse = null;
                    }
                }
            }
        }

        // If the final message is not from user, append a user message saying "Continue."
        if (!empty($messages) && end($messages)['role'] !== 'user') {
            $messages[] = [
                'role' => 'user',
                'content' => [['text' => 'Continue.']]
            ];
        }

        // Validate all messages have non-empty content
        foreach ($messages as $message) {
            if (empty($message['content'])) {
                Log::error('Empty content in message', ['message' => $message]);
                throw new \Exception('Message content cannot be empty');
            }
        }

        Log::info('Converted messages', ['messages' => $messages]);

        return [
            'system' => $system,
            'messages' => $messages
        ];
    }

    private function formatContent($content): array
    {
        // If content is empty, return a default text block
        if (empty($content)) {
            return [['text' => ' ']];
        }

        // If content is a string, wrap it in a text block
        if (is_string($content)) {
            return [['text' => $content]];
        }

        // If content is already an array of content blocks
        if (is_array($content) && !empty($content) && isset($content[0]) && 
            (isset($content[0]['text']) || isset($content[0]['toolResult']) || isset($content[0]['toolUse']))) {
            return array_map(function($block) {
                if (isset($block['text']) && empty($block['text'])) {
                    $block['text'] = ' ';
                }
                return $block;
            }, $content);
        }

        // If content is an array but not in content block format
        if (is_array($content)) {
            $formatted = [];
            foreach ($content as $item) {
                if (is_string($item)) {
                    $formatted[] = ['text' => $item];
                } elseif (isset($item['type']) && $item['type'] === 'tool-call') {
                    $formatted[] = [
                        'toolUse' => [
                            'toolUseId' => $item['toolCallId'],
                            'name' => $item['toolName'],
                            'input' => $item['args']
                        ]
                    ];
                } elseif (isset($item['toolResult'])) {
                    $formatted[] = $item;
                } else {
                    $formatted[] = ['text' => json_encode($item)];
                }
            }
            return $formatted;
        }

        // Fallback: convert to JSON string and wrap in text block
        return [['text' => json_encode($content)]];
    }
}