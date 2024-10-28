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

        $blocks = $this->groupIntoBlocks($prompt);

        $system = null;
        $messages = [];

        foreach ($blocks as $i => $block) {
            $type = $block['type'];

            Log::debug("[BedrockMessageConverter] Processing block", ['type' => $type, 'block' => json_encode($block, JSON_PRETTY_PRINT)]);

            switch ($type) {
                case 'system':
                    if (!empty($messages)) {
                        throw new \Exception('Multiple system messages that are separated by user/assistant messages are not supported.');
                    }
                    $system = implode("\n", array_map(fn($msg) => $msg['content'], $block['messages']));
                    break;

                case 'user':
                    $bedrockContent = [];
                    foreach ($block['messages'] as $message) {
                        $role = $message['role'];
                        $content = $message['content'];

                        Log::debug("[BedrockMessageConverter] Processing user message", ['role' => $role, 'content' => json_encode($content, JSON_PRETTY_PRINT)]);

                        switch ($role) {
                            case 'user':
                                if (is_string($content)) {
                                    $bedrockContent[] = ['text' => $content];
                                } elseif (is_array($content)) {
                                    foreach ($content as $part) {
                                        if (is_array($part) && isset($part['type']) && $part['type'] === 'text') {
                                            $bedrockContent[] = ['text' => $part['text']];
                                        } elseif (is_string($part)) {
                                            $bedrockContent[] = ['text' => $part];
                                        }
                                    }
                                } else {
                                    throw new \Exception("Unsupported content type for user message");
                                }
                                break;
                            case 'tool':
                                foreach ($content as $part) {
                                    $bedrockContent[] = [
                                        'toolResult' => [
                                            'toolUseId' => $part['toolCallId'],
                                            'content' => [['text' => json_encode($part['result'])]]
                                        ]
                                    ];
                                }
                                break;
                            default:
                                throw new \Exception("Unsupported role: {$role}");
                        }
                    }
                    $messages[] = ['role' => 'user', 'content' => $bedrockContent];
                    break;

                case 'assistant':
                    $bedrockContent = [];
                    foreach ($block['messages'] as $j => $message) {
                        $content = $message['content'];
                        Log::debug("[BedrockMessageConverter] Processing assistant message", ['content' => json_encode($content, JSON_PRETTY_PRINT)]);

                        if (is_string($content)) {
                            $bedrockContent[] = ['text' => $content];
                        } elseif (is_array($content)) {
                            foreach ($content as $k => $part) {
                                if (is_array($part) && isset($part['type'])) {
                                    switch ($part['type']) {
                                        case 'text':
                                            $text = $part['text'];
                                            if ($i === count($blocks) - 1 && $j === count($block['messages']) - 1) {
                                                $text = trim($text);
                                            }
                                            $bedrockContent[] = ['text' => $text];
                                            break;
                                        case 'tool-call':
                                            $bedrockContent[] = [
                                                'toolUse' => [
                                                    'toolUseId' => $part['toolCallId'],
                                                    'name' => $part['toolName'],
                                                    'input' => $part['args']
                                                ]
                                            ];
                                            break;
                                    }
                                } elseif (is_string($part)) {
                                    $bedrockContent[] = ['text' => $part];
                                }
                            }
                        } else {
                            throw new \Exception("Unsupported content type for assistant message");
                        }

                        // Handle toolInvocations
                        if (isset($message['toolInvocations']) && is_array($message['toolInvocations'])) {
                            Log::debug("[BedrockMessageConverter] Processing toolInvocations", ['toolInvocations' => json_encode($message['toolInvocations'], JSON_PRETTY_PRINT)]);
                            foreach ($message['toolInvocations'] as $toolInvocation) {
                                if ($toolInvocation['state'] === 'result') {
                                    $bedrockContent[] = [
                                        'toolResult' => [
                                            'toolUseId' => $toolInvocation['toolCallId'],
                                            'content' => [['text' => json_encode($toolInvocation['result'])]]
                                        ]
                                    ];
                                }
                            }
                        }
                    }
                    $messages[] = ['role' => 'assistant', 'content' => $bedrockContent];
                    break;

                default:
                    throw new \Exception("Unsupported type: {$type}");
            }
        }

        Log::info('[BedrockMessageConverter] Converted messages', [
            'messages' => json_encode($messages, JSON_PRETTY_PRINT)
        ]);

        // If the final message is not from user, append a user message saying "Continue."
        if (!empty($messages) && $messages[count($messages) - 1]['role'] !== 'user') {
            $messages[] = ['role' => 'user', 'content' => [['text' => 'Continue.']]];
        } else {
            Log::debug("[BedrockMessageConverter] Didn't append 'Continue' message", [
                'last_message' => json_encode($messages[count($messages) - 1], JSON_PRETTY_PRINT)
            ]);
        }

        return [
            'system' => $system,
            'messages' => $messages
        ];
    }

    private function groupIntoBlocks(array $prompt): array
    {
        $blocks = [];
        $currentBlock = null;

        foreach ($prompt as $message) {
            $role = $message['role'];

            switch ($role) {
                case 'system':
                case 'assistant':
                    if (!isset($currentBlock) || $currentBlock['type'] !== $role) {
                        $currentBlock = ['type' => $role, 'messages' => []];
                        $blocks[] = &$currentBlock;
                    }
                    $currentBlock['messages'][] = $message;
                    break;
                case 'user':
                case 'tool':
                    if (!isset($currentBlock) || $currentBlock['type'] !== 'user') {
                        $currentBlock = ['type' => 'user', 'messages' => []];
                        $blocks[] = &$currentBlock;
                    }
                    $currentBlock['messages'][] = $message;
                    break;
                default:
                    throw new \Exception("Unsupported role: {$role}");
            }
        }

        return $blocks;
    }
}