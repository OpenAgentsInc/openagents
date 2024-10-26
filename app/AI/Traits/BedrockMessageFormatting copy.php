<?php

declare(strict_types=1);

namespace App\AI\Traits;

use Illuminate\Support\Facades\Log;

trait BedrockMessageFormattingNo
{
    /**
     * Convert the input prompt to Bedrock chat messages format.
     *
     * @param array $prompt The input prompt array
     * @return array The formatted Bedrock messages prompt
     */
    private function convertToBedrockChatMessages(array $prompt): array
    {
        $system = null;
        $messages = [];
        $firstUserMessageFound = false;

        foreach ($prompt as $message) {
            $role = $message['role'];
            $content = $message['content'];

            switch ($role) {
                case 'system':
                    if ($firstUserMessageFound) {
                        throw new \Exception('System messages must come before any user or assistant messages');
                    }
                    $system = $content;
                    break;

                case 'user':
                    $firstUserMessageFound = true;
                    $bedrockContent = $this->formatUserContent([$message]);
                    $messages[] = ['role' => 'user', 'content' => $bedrockContent];
                    break;

                case 'assistant':
                    if (!$firstUserMessageFound) {
                        continue; // Skip assistant messages before the first user message
                    }
                    $bedrockContent = $this->formatAssistantContent([$message], false);
                    $messages[] = ['role' => 'assistant', 'content' => $bedrockContent];
                    break;

                case 'tool':
                    if (!$firstUserMessageFound) {
                        continue; // Skip tool messages before the first user message
                    }
                    $toolContent = $this->formatToolContent([$message]);
                    // Append tool content to the last user message
                    if (!empty($messages) && end($messages)['role'] === 'user') {
                        $lastIndex = count($messages) - 1;
                        $messages[$lastIndex]['content'] = array_merge($messages[$lastIndex]['content'], $toolContent);
                    } else {
                        // If there's no preceding user message, create a new one
                        $messages[] = ['role' => 'user', 'content' => $toolContent];
                    }
                    break;

                default:
                    throw new \Exception("Unsupported role: $role");
            }
        }

        // Ensure the conversation starts with a user message
        if (empty($messages) || $messages[0]['role'] !== 'user') {
            array_unshift($messages, [
                'role' => 'user',
                'content' => [['text' => 'Hello']]
            ]);
        }

        // If the last message is from the assistant, append a user message
        if (!empty($messages) && end($messages)['role'] === 'assistant') {
            $messages[] = [
                'role' => 'user',
                'content' => [['text' => 'Please continue.']]
            ];
        }

        return [
            'system' => $system,
            'messages' => $messages,
        ];
    }

    /**
     * Format user content for Bedrock.
     *
     * @param array $messages An array of user messages
     * @return array Formatted Bedrock content
     */
    private function formatUserContent(array $messages): array
    {
        $bedrockContent = [];

        foreach ($messages as $message) {
            $content = $message['content'];

            if (is_string($content)) {
                $bedrockContent[] = ['text' => $content];
            } elseif (is_array($content)) {
                foreach ($content as $part) {
                    switch ($part['type']) {
                        case 'text':
                            $bedrockContent[] = ['text' => $part['text']];
                            break;
                        case 'image':
                            $bedrockContent[] = [
                                'image' => [
                                    'format' => $part['mimeType'] ? explode('/', $part['mimeType'])[1] : null,
                                    'source' => [
                                        'bytes' => $part['image'],
                                    ],
                                ],
                            ];
                            break;
                    }
                }
            }
        }

        return $bedrockContent;
    }

    /**
     * Format tool content for Bedrock.
     *
     * @param array $messages An array of tool messages
     * @return array Formatted Bedrock content
     */
    private function formatToolContent(array $messages): array
    {
        $bedrockContent = [];

        foreach ($messages as $message) {
            $content = $message['content'];

            if (is_array($content)) {
                foreach ($content as $part) {
                    $bedrockContent[] = [
                        'toolResult' => [
                            'toolUseId' => $part['toolCallId'],
                            'content' => [['text' => json_encode($part['result'])]],
                        ],
                    ];
                }
            }
        }

        return $bedrockContent;
    }

    /**
     * Format assistant content for Bedrock.
     *
     * @param array $messages An array of assistant messages
     * @param bool $isLastBlock Whether this is the last block in the conversation
     * @return array Formatted Bedrock content
     */
    private function formatAssistantContent(array $messages, bool $isLastBlock): array
    {
        $bedrockContent = [];

        foreach ($messages as $index => $message) {
            $content = $message['content'];

            if (is_string($content)) {
                $bedrockContent[] = ['text' => $content];
            } elseif (is_array($content)) {
                foreach ($content as $partIndex => $part) {
                    switch ($part['type']) {
                        case 'text':
                            $bedrockContent[] = ['text' => $part['text']];
                            break;
                        case 'tool-call':
                            $bedrockContent[] = [
                                'toolUse' => [
                                    'toolUseId' => $part['toolCallId'],
                                    'name' => $part['toolName'],
                                    'input' => $part['args'],
                                ],
                            ];
                            break;
                    }
                }
            }
        }

        return $bedrockContent;
    }

    /**
     * Format the response from Bedrock API.
     *
     * @param array $decodedBody The decoded response body from Bedrock API
     * @return array Formatted response
     */
    private function formatResponse(array $decodedBody): array
    {
        $response = $this->initializeResponse($decodedBody);

        if (isset($decodedBody['output']['message']['content'])) {
            foreach ($decodedBody['output']['message']['content'] as $contentItem) {
                $this->processContentItem($contentItem, $response);
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
    private function initializeResponse(array $decodedBody): array
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
    private function processContentItem(array $contentItem, array &$response): void
    {
        if (isset($contentItem['text'])) {
            $response['content'] .= $contentItem['text'];
        } elseif (isset($contentItem['toolUse'])) {
            $response['toolInvocations'][] = $this->formatToolInvocation($contentItem['toolUse']);
        }
    }

    /**
     * Format a tool invocation for the response.
     *
     * @param array $toolUse The tool use data from Bedrock API
     * @return array Formatted tool invocation
     */
    private function formatToolInvocation(array $toolUse): array
    {
        return [
            'toolName' => $toolUse['name'],
            'toolCallId' => $toolUse['toolUseId'],
            'args' => $toolUse['input'],
        ];
    }
}
