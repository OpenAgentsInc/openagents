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

            // Handle tool invocations before adding the message content
            if ($message['role'] === 'assistant' && isset($message['toolInvocations'])) {
                foreach ($message['toolInvocations'] as $toolInvocation) {
                    // Add the tool use message
                    $messages[] = [
                        'role' => 'assistant',
                        'content' => [
                            [
                                'toolUse' => [
                                    'toolUseId' => $toolInvocation['toolCallId'],
                                    'name' => $toolInvocation['toolName'],
                                    'input' => $toolInvocation['args']
                                ]
                            ]
                        ]
                    ];

                    // Add the tool result message if it's a result
                    if ($toolInvocation['state'] === 'result') {
                        $messages[] = [
                            'role' => 'user',
                            'content' => [
                                [
                                    'toolResult' => [
                                        'toolUseId' => $toolInvocation['toolCallId'],
                                        'status' => $toolInvocation['result']['value']['result']['success'] ? 'success' : 'error',
                                        'content' => [
                                            [
                                                'text' => json_encode($toolInvocation['result'])
                                            ]
                                        ]
                                    ]
                                ]
                            ]
                        ];
                    }
                }
                
                // Skip adding the original message content if it was just a tool invocation
                // or if the content is empty/whitespace
                $isEmpty = is_string($message['content']) ? 
                    empty(trim($message['content'])) : 
                    empty($message['content']);
                
                if ($isEmpty) {
                    $lastRole = 'user'; // Set to user since we added a tool result
                    continue;
                }
            }

            // Skip empty assistant messages that don't have tool invocations
            if ($message['role'] === 'assistant' && 
                is_string($message['content']) && 
                empty(trim($message['content'])) && 
                empty($message['toolInvocations'])) {
                continue;
            }

            $formattedContent = $this->formatContent($message['content']);
            
            // Skip if formatted content is empty
            if (empty($formattedContent)) {
                continue;
            }

            // Ensure no empty text blocks
            $formattedContent = array_filter($formattedContent, function($block) {
                if (isset($block['text'])) {
                    return !empty(trim($block['text']));
                }
                return true;
            });

            // Skip if all content blocks were empty
            if (empty($formattedContent)) {
                continue;
            }

            $messages[] = [
                'role' => $message['role'],
                'content' => array_values($formattedContent) // Re-index array after filtering
            ];
            $lastRole = $message['role'];
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
        // If content is empty, return empty array (will be filtered out)
        if (empty($content)) {
            return [];
        }

        // If content is a string, wrap it in a text block if not empty
        if (is_string($content)) {
            $trimmed = trim($content);
            return empty($trimmed) ? [] : [['text' => $trimmed]];
        }

        // If content is already an array of content blocks
        if (is_array($content) && !empty($content) && isset($content[0]) && 
            (isset($content[0]['text']) || isset($content[0]['toolResult']) || isset($content[0]['toolUse']) || isset($content[0]['type']))) {
            $formatted = [];
            foreach ($content as $block) {
                if (isset($block['type']) && $block['type'] === 'tool-call') {
                    $formatted[] = [
                        'toolUse' => [
                            'toolUseId' => $block['toolCallId'],
                            'name' => $block['toolName'],
                            'input' => $block['args']
                        ]
                    ];
                } elseif (isset($block['text'])) {
                    $trimmed = trim($block['text']);
                    if (!empty($trimmed)) {
                        $formatted[] = ['text' => $trimmed];
                    }
                } else {
                    $formatted[] = $block;
                }
            }
            return $formatted;
        }

        // If content is an array but not in content block format
        if (is_array($content)) {
            $formatted = [];
            foreach ($content as $item) {
                if (is_string($item)) {
                    $trimmed = trim($item);
                    if (!empty($trimmed)) {
                        $formatted[] = ['text' => $trimmed];
                    }
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