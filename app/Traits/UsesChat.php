<?php

namespace App\Traits;

use App\Models\Message;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Validator;
use Illuminate\Validation\ValidationException;

trait UsesChat
{
    private $validatedData;
    private $request;
    private $userMessage;
    private $assistantMessage;
    private $tools;
    private $callback;
    private $response;

    private function createChatStream(Request $request)
    {
        $this->request = $request;
        $this->validatedData = $this->validateChatRequest();
        $this->userMessage = $this->saveUserMessage();
        $this->tools = $this->getTools();
        $this->callback = $this->createChatCallback();

        return $this->createStreamedResponse();
    }

    private function formatMessageContent($content)
    {
        if (empty($content)) {
            return [['text' => ' ']]; // Never return empty content
        }

        if (is_string($content)) {
            return [['text' => $content]];
        }

        // If it's already in the correct format with text or toolResult
        if (is_array($content) && isset($content[0]) && (isset($content[0]['text']) || isset($content[0]['toolResult']))) {
            return array_map(function ($block) {
                if (empty($block['text']) && !isset($block['toolResult'])) {
                    $block['text'] = ' '; // Ensure text is never empty
                }
                return $block;
            }, $content);
        }

        // If it's a single content block
        if (is_array($content) && !isset($content[0])) {
            if (isset($content['text'])) {
                return [$content];
            }
            if (isset($content['toolResult'])) {
                return [$content];
            }
            return [['text' => json_encode($content)]];
        }

        // If it's an array of strings or other values
        if (is_array($content)) {
            return array_map(function ($item) {
                if (is_string($item)) {
                    return ['text' => $item];
                }
                return ['text' => json_encode($item)];
            }, $content);
        }

        // Fallback
        return [['text' => json_encode($content)]];
    }

    private function handleToolInvocations()
    {
        if (!isset($this->response['toolInvocations'])) {
            Log::info("No tool invocations in response", ['response' => $this->response]);
            $this->streamFinishEvent('stop');
            return;
        }

        $toolInvocations = $this->response['toolInvocations'];
        Log::info("Found tool invocations in response", ['toolInvocations' => $toolInvocations]);

        if (empty($toolInvocations)) {
            $this->streamFinishEvent('stop');
            return;
        }

        $messages = $this->validatedData['messages'];
        $toolResults = [];

        foreach ($toolInvocations as $toolInvocation) {
            $this->streamToolCall([$toolInvocation]);
            $this->streamFinishEvent('tool-calls');

            $toolResult = $this->toolService->handleToolCall($toolInvocation, $this->assistantMessage->id);
            $formattedToolResult = [
                'toolCallId' => $toolResult['toolCallId'] ?? $toolInvocation['toolCallId'] ?? null,
                'result' => $toolResult['result'] ?? $toolResult,
            ];

            $this->streamToolResult($formattedToolResult);

            // Add tool result to messages
            $result = $toolResult['result'] ?? $toolResult;
            if (isset($result['value']) && isset($result['value']['result'])) {
                $result = $result['value']['result'];
            }

            $toolResults[] = [
                'toolResult' => [
                    'toolUseId' => $formattedToolResult['toolCallId'],
                    'status' => isset($result['success']) && $result['success'] ? 'success' : 'error',
                    'content' => [
                        [
                            'text' => isset($result['content']) ? $result['content'] : json_encode($result)
                        ]
                    ]
                ]
            ];
        }

        // // Add tool results to messages
        // if (!empty($toolResults)) {
        //     // Convert tool results to text blocks
        //     $textBlocks = [['text' => $this->response['content'] ?: ' ']];
        //     foreach ($toolResults as $toolResult) {
        //         $textBlocks[] = [
        //             'text' => "Tool result: " . json_encode($toolResult['toolResult'])
        //         ];
        //     }

        //     $messages[] = [
        //         'role' => 'assistant',
        //         'content' => $textBlocks
        //     ];

        //     // Format all messages to ensure proper content structure
        //     // $formattedMessages = array_map(function ($message) {
        //     //     $formatted = [
        //     //         'role' => $message['role'],
        //     //         'content' => $this->formatMessageContent($message['content'])
        //     //     ];
        //     //     Log::debug('Formatted message', ['original' => $message, 'formatted' => $formatted]);
        //     //     return $formatted;
        //     // }, $messages);

        //     // Log::info('Making inference call with messages', ['messages' => $formattedMessages]);

        //     // // Make a new inference call with updated messages
        //     // $this->response = $this->gateway->inference([
        //     //     'model' => $this->model,
        //     //     'messages' => $formattedMessages,
        //     //     'tools' => $this->tools,
        //     //     'max_tokens' => 4096,
        //     // ]);

        //     // // Stream the new response
        //     // if (!empty($this->response['content'])) {
        //     //     $content = $this->response['content'];
        //     //     $words = explode(' ', $content);
        //     //     foreach ($words as $word) {
        //     //         $this->stream($word . ' ');
        //     //         usleep(50000);
        //     //     }
        //     // }
        // }

        $this->streamFinishEvent('stop');
    }

    private function createChatCallback()
    {
        // Format messages to ensure proper alternation and content structure
        $messages = $this->validatedData['messages'];
        $formattedMessages = [];
        $lastRole = null;

        foreach ($messages as $message) {
            if ($lastRole === $message['role']) {
                // If same role appears twice, combine their content
                $lastMessage = end($formattedMessages);
                $lastContent = $this->formatMessageContent($lastMessage['content']);
                $newContent = $this->formatMessageContent($message['content']);
                $formattedMessages[key($formattedMessages)]['content'] = array_merge($lastContent, $newContent);
            } else {
                $message['content'] = $this->formatMessageContent($message['content']);
                $formattedMessages[] = $message;
                $lastRole = $message['role'];
            }
        }

        Log::info('Making initial inference call with messages', ['messages' => $formattedMessages]);

        $this->response = $this->gateway->inference([
            'model' => $this->model,
            'messages' => $formattedMessages,
            'tools' => $this->tools,
            'max_tokens' => 4096,
        ]);

        $this->storeAIResponse();

        return function () {
            // Initial empty text delta
            $this->stream(' ');

            // Handle tool invocations first
            if (!empty($this->response['toolInvocations'])) {
                $this->handleToolInvocations();
            } else {
                // Stream the content if no tools were used
                if (!empty($this->response['content'])) {
                    $content = $this->response['content'];
                    $words = explode(' ', $content);
                    foreach ($words as $word) {
                        $this->stream($word . ' ');
                        usleep(50000);
                    }
                }
                $this->streamFinishEvent('stop');
            }
        };
    }

    private function validateChatRequest()
    {
        $request = $this->request;
        $validator = Validator::make($request->all(), [
            'messages' => 'required|array',
            'messages.*.role' => 'required|string|in:user,assistant,system',
            'messages.*.content' => 'required_without:messages.*.toolInvocations',
            'messages.*.content.*' => 'sometimes|array',
            'messages.*.content.*.type' => 'sometimes|string|in:text,tool-call',
            'messages.*.content.*.text' => 'required_if:messages.*.content.*.type,text|string',
            'messages.*.content.*.toolCallId' => 'required_if:messages.*.content.*.type,tool-call|string',
            'messages.*.content.*.toolName' => 'required_if:messages.*.content.*.type,tool-call|string',
            'messages.*.content.*.args' => 'required_if:messages.*.content.*.type,tool-call|array',
            'messages.*.toolInvocations' => 'sometimes|array',
            'messages.*.toolInvocations.*.state' => 'required_with:messages.*.toolInvocations|string',
            'messages.*.toolInvocations.*.toolCallId' => 'required_with:messages.*.toolInvocations|string',
            'messages.*.toolInvocations.*.toolName' => 'required_with:messages.*.toolInvocations|string',
            'messages.*.toolInvocations.*.args' => 'required_with:messages.*.toolInvocations|array',
            'messages.*.toolInvocations.*.result' => 'required_with:messages.*.toolInvocations',
            'selected_tools' => 'sometimes|array',
            'selected_tools.*' => 'string|in:view_file,view_folder,rewrite_file,create_file',
        ]);

        if ($validator->fails()) {
            Log::warning('Validation failed', ['errors' => $validator->errors()]);
            throw new ValidationException($validator);
        }

        $validated = $validator->validated();
        Log::info('Validated chat request', ['request' => $validated]);

        // Ensure each message has properly formatted content
        $validated['messages'] = array_map(function ($message) {
            $message['content'] = $this->formatMessageContent($message['content']);
            return $message;
        }, $validated['messages']);

        return $validated;
    }

    private function saveUserMessage()
    {
        $validatedData = $this->validatedData;
        $user = $this->request->user();
        $messages = $validatedData['messages'];
        $thread_id = 1; // TODO
        $lastMessage = end($messages);

        // Convert array content to string for storage
        $content = is_array($lastMessage['content'])
            ? json_encode($lastMessage['content'])
            : ($lastMessage['content'] ?? "(empty)");

        $messageData = [
            'content' => $content,
            'thread_id' => $thread_id,
            'team_id' => null,
            'user_id' => 1, // $user->id,
            'role' => 'user',
        ];

        Log::info('Saving user chat message', ['messageData' => $messageData]);

        return Message::create($messageData);
    }

    private function getTools()
    {
        $selectedTools = $this->validatedData['selected_tools'] ?? [];
        $tools = empty($selectedTools) ? [] : $this->toolService->getToolDefinitions($selectedTools);
        return $tools;
    }

    private function storeAIResponse()
    {
        $content = is_array($this->response['content'])
            ? json_encode($this->response['content'])
            : ($this->response['content'] ?? "(empty)");

        $assistantMessage = [
            'content' => $content,
            'thread_id' => 1,
            'team_id' => $this->userMessage->team_id,
            'input_tokens' => $this->response['input_tokens'] ?? 0,
            'output_tokens' => $this->response['output_tokens'] ?? 0,
            'model' => $this->model,
            'role' => 'assistant',
            'is_system_message' => true,
        ];

        Log::info('AI response received. Saving message', ['response' => $this->response, 'assistantMessage' => $assistantMessage]);

        $this->assistantMessage = Message::create($assistantMessage);
    }
}
