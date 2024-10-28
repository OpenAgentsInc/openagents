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

        $this->streamToolCall($toolInvocations);
        $this->streamFinishEvent('tool-calls');

        foreach ($toolInvocations as $toolInvocation) {
            $toolResult = $this->toolService->handleToolCall($toolInvocation, $this->assistantMessage->id);

            // Ensure the toolResult has the correct structure
            $formattedToolResult = [
                'toolCallId' => $toolResult['toolCallId'] ?? $toolInvocation['toolCallId'] ?? null,
                'result' => $toolResult['result'] ?? $toolResult,
            ];

            $this->streamToolResult($formattedToolResult);
        }

        $this->streamFinishEvent('stop');
    }

    private function createChatCallback()
    {
        $this->response = $this->gateway->inference([
            'model' => $this->model,
            'messages' => $this->validatedData['messages'],
            'tools' => $this->tools,
            'max_tokens' => 4096,
        ]);

        $this->storeAIResponse();

        return function () {
            // Initial response structure
            $this->stream(' '); // Initial text delta
            
            if (!empty($this->response['toolInvocations'])) {
                $firstToolCall = reset($this->response['toolInvocations']);
                $this->streamToolCall([$firstToolCall]);
                $this->streamFinishEvent('tool-calls', [
                    'promptTokens' => $this->response['input_tokens'] ?? 0,
                    'completionTokens' => $this->response['output_tokens'] ?? 0
                ]);
                
                $toolResult = $this->toolService->handleToolCall($firstToolCall, $this->assistantMessage->id);
                $formattedToolResult = [
                    'toolCallId' => $toolResult['toolCallId'] ?? $firstToolCall['toolCallId'] ?? null,
                    'result' => $toolResult['result'] ?? $toolResult,
                ];
                $this->streamToolResult($formattedToolResult);
            }

            // Stream the content word by word
            $content = $this->response['content'];
            $words = explode(' ', $content);
            foreach ($words as $word) {
                $this->stream($word . ' ');
                usleep(50000); // Sleep for 50ms between words
            }

            // Handle remaining tool invocations if any
            if (!empty($this->response['toolInvocations']) && count($this->response['toolInvocations']) > 1) {
                $remainingTools = array_slice($this->response['toolInvocations'], 1);
                foreach ($remainingTools as $toolCall) {
                    $this->streamToolCall([$toolCall]);
                    $this->streamFinishEvent('tool-calls');
                    
                    $toolResult = $this->toolService->handleToolCall($toolCall, $this->assistantMessage->id);
                    $formattedToolResult = [
                        'toolCallId' => $toolResult['toolCallId'] ?? $toolCall['toolCallId'] ?? null,
                        'result' => $toolResult['result'] ?? $toolResult,
                    ];
                    $this->streamToolResult($formattedToolResult);
                }
            }

            $this->streamFinishEvent('stop');
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
        } else {
            Log::info('Validated chat request', ['request' => $request->all()]);
        }

        return $validator->validated();
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
        ];

        Log::info('AI response received. Saving message', ['response' => $this->response, 'assistantMessage' => $assistantMessage]);

        $this->assistantMessage = Message::create($assistantMessage);
    }
}