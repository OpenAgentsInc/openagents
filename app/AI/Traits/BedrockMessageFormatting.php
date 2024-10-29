<?php

declare(strict_types=1);

namespace App\AI\Traits;

use Illuminate\Support\Facades\Log;

trait BedrockMessageFormatting
{
    /**
     * Convert the input prompt to Bedrock chat messages format.
     *
     * @param array $prompt The input prompt array
     * @return array The formatted Bedrock messages prompt
     */
    protected function convertToBedrockChatMessages(array $messages): array
    {
        Log::info('[New] Converting prompt to Bedrock chat messages', [
            'messages' => json_encode($messages, JSON_PRETTY_PRINT)
        ]);

        $formattedMessages = [];

        foreach ($messages as $message) {
            $messageToAdd = [
                'role' => $message['role'],
                'content' => [
                    [
                        'text' => $message['content']
                    ]
                ]
            ];

            // If there is a toolInvocation with state of 'result', add a toolResult
            if (isset($message['toolInvocations'])) {
                $toolInvocation = $message['toolInvocations'][0];

                if ($toolInvocation['state'] === 'result') {
                    $messageToAdd['toolResult'] = [
                        'content' => [
                            [
                                'text' => json_encode($toolInvocation['result'])
                            ]
                        ],
                        'status' => $this->determineToolResultStatus($toolInvocation['result']),
                        'toolUseId' => $toolInvocation['toolCallId']
                    ];
                }
            }

            $formattedMessages[] = $messageToAdd;
        }

        Log::info('[New] Finished', [
            '$formattedMessages' => json_encode($formattedMessages, JSON_PRETTY_PRINT)
        ]);

        return [
            'messages' => $formattedMessages,
            'system' => [
                [
                    'text' => 'You are a helpful AI agent on OpenAgents.com.'
                ]
            ]
        ];
    }

    /**
     * Convert the input prompt to Bedrock chat messages format.
     *
     * @param array $prompt The input prompt array
     * @return array The formatted Bedrock messages prompt
     */
    protected function convertToBedrockChatMessagesOld(array $messages): array
    {
        Log::info('Converting prompt to Bedrock chat messages', [
            'messages' => json_encode($messages, JSON_PRETTY_PRINT)
        ]);

        $formattedMessages = [];
        $pendingToolUses = [];

        foreach ($messages as $index => $message) {
            $role = $message['role'];
            $content = $message['content'];
            $toolInvocations = $message['toolInvocations'] ?? [];

            $formattedContent = [
                [
                    'text' => $content
                ]
            ];

            // Process tool invocations
            foreach ($toolInvocations as $toolInvocation) {
                if ($toolInvocation['state'] === 'result') {
                    // Match tool result with a pending tool use
                    if (!empty($pendingToolUses)) {
                        $toolUse = array_shift($pendingToolUses);
                        $formattedContent[] = [
                            'toolResult' => [
                                'content' => [
                                    [
                                        'text' => json_encode($toolInvocation['result'])
                                    ]
                                ],
                                'status' => 'success', // $this->determineToolResultStatus($toolInvocation['result']),
                                'toolUseId' => $toolUse['toolUseId']
                            ]
                        ];
                    }
                } elseif ($role === 'user') {
                    // Store tool uses for the next turn
                    $pendingToolUses[] = [
                        'toolUse' => [
                            'input' => $toolInvocation['args'] ?? [],
                            'name' => $toolInvocation['toolName'] ?? 'unknown',
                            'toolUseId' => $toolInvocation['toolCallId'] ?? 'unknown'
                        ]
                    ];
                    $formattedContent[] = $pendingToolUses[count($pendingToolUses) - 1];
                }
            }

            $formattedMessages[] = [
                'content' => $formattedContent,
                'role' => $role
            ];
        }

        // Ensure the first message is from a user
        if (empty($formattedMessages) || $formattedMessages[0]['role'] !== 'user') {
            array_unshift($formattedMessages, [
                'content' => [['text' => 'Hello']],
                'role' => 'user'
            ]);
        }

        // Ensure the last message is from a user
        if (end($formattedMessages)['role'] !== 'user') {
            $formattedMessages[] = [
                'content' => [['text' => 'Continue.']],
                'role' => 'user'
            ];
        }

        $finalMessages = [
            "system" => [
                [
                    "text" => "You are a helpful AI agent on OpenAgents.com"
                ]
            ],
            "messages" => $formattedMessages
        ];

        Log::info("Returning formatted Bedrock messages", ['returning' => json_encode($finalMessages, JSON_PRETTY_PRINT)]);

        return $finalMessages;
    }

    /**
     * Determine the status of a tool result.
     *
     * @param array $result The tool result array
     * @return string The determined status ('success' or 'error')
     */
    protected function determineToolResultStatus(array $result): string
    {
        // Check for explicit success key
        if (isset($result['success'])) {
            return $result['success'] ? 'success' : 'error';
        }

        // Check for error key or message
        if (isset($result['error']) || isset($result['errorMessage'])) {
            return 'error';
        }

        // If there's content and no error indicators, assume success
        if (isset($result['content']) && !empty($result['content'])) {
            return 'success';
        }

        // Default to 'error' if we can't determine the status
        return 'error';
    }

    /**
     * Format the response from Bedrock API.
     *
     * @param array $decodedBody The decoded response body from Bedrock API
     * @return array Formatted response
     */
    protected function formatResponse(array $decodedBody): array
    {
        $response = $this->initializeResponse($decodedBody);
        $foundToolUse = false;

        if (isset($decodedBody['output']['message']['content'])) {
            foreach ($decodedBody['output']['message']['content'] as $contentItem) {
                // If we've found a tool use, only process tool-related items
                if ($foundToolUse) {
                    if (isset($contentItem['toolUse'])) {
                        $this->processContentItem($contentItem, $response);
                    }
                    continue;
                }

                $this->processContentItem($contentItem, $response);
                
                // If this was a tool use, set the flag
                if (isset($contentItem['toolUse'])) {
                    $foundToolUse = true;
                }
            }
        }

        // Handle tool results if present
        if (isset($decodedBody['output']['message']['toolResults'])) {
            foreach ($decodedBody['output']['message']['toolResults'] as $toolResult) {
                $this->processToolResult($toolResult, $response);
            }
        }

        Log::info('Processed response', ['response' => $response]);
        return $response;
    }

    /**
     * Initialize the response array with basic structure.
     *
     * @param array $decodedBody The decoded response body from Bedrock API
     * @return array Initialized response array
     */
    protected function initializeResponse(array $decodedBody): array
    {
        return [
            'content' => '',
            'input_tokens' => $decodedBody['usage']['inputTokens'] ?? 0,
            'output_tokens' => $decodedBody['usage']['outputTokens'] ?? 0,
            'toolInvocations' => [],
        ];
    }

    /**
     * Process a single content item from the Bedrock API response.
     *
     * @param array $contentItem The content item to process
     * @param array &$response The response array to update
     */
    protected function processContentItem(array $contentItem, array &$response): void
    {
        if (isset($contentItem['text'])) {
            $response['content'] .= $contentItem['text'];
        } elseif (isset($contentItem['toolUse'])) {
            $response['toolInvocations'][] = $this->formatToolInvocation($contentItem['toolUse']);
        }
    }

    /**
     * Process a tool result from the Bedrock API response.
     *
     * @param array $toolResult The tool result to process
     * @param array &$response The response array to update
     */
    protected function processToolResult(array $toolResult, array &$response): void
    {
        if (isset($toolResult['toolUseId'])) {
            foreach ($response['toolInvocations'] as &$invocation) {
                if ($invocation['toolCallId'] === $toolResult['toolUseId']) {
                    $invocation['result'] = [
                        'type' => 'tool_call',
                        'value' => [
                            'toolCallId' => $toolResult['toolUseId'],
                            'toolName' => $invocation['toolName'],
                            'args' => $invocation['args'],
                            'result' => [
                                'success' => $toolResult['status'] === 'success',
                                'content' => $toolResult['content'][0]['text'] ?? null,
                                'error' => $toolResult['status'] === 'error' ? ($toolResult['content'][0]['text'] ?? 'Unknown error') : null
                            ]
                        ]
                    ];
                    break;
                }
            }
        }
    }

    /**
     * Format a tool invocation for the response.
     *
     * @param array $toolUse The tool use data from Bedrock API
     * @return array Formatted tool invocation
     */
    protected function formatToolInvocation(array $toolUse): array
    {
        return [
            'toolName' => $toolUse['name'],
            'toolCallId' => $toolUse['toolUseId'],
            'args' => $toolUse['input'],
        ];
    }
}