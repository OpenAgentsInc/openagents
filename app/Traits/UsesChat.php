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
            return;
        }

        $toolInvocations = $this->response['toolInvocations'];
        Log::info("Found tool invocations in response", ['toolInvocations' => $toolInvocations]);

        if (empty($toolInvocations)) {
            return;
        }

        $this->streamToolCall($toolInvocations);

        foreach ($toolInvocations as $toolInvocation) {
            $toolResult = $this->toolService->handleToolCall($toolInvocation, $this->assistantMessage->id);

            // Ensure the toolResult has the correct structure
            $formattedToolResult = [
                'toolCallId' => $toolResult['toolCallId'] ?? $toolInvocation['toolCallId'] ?? null,
                'result' => $toolResult['result'] ?? $toolResult,
            ];

            $this->streamToolResult($formattedToolResult);
        }
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
            // Stream the content word by word
            $content = $this->response['content'];
            $words = explode(' ', $content);
            foreach ($words as $word) {
                $this->stream($word . ' ');
                usleep(50000); // Sleep for 50ms between words
            }

            // Add a small delay before handling tool invocations
            usleep(100000); // 100ms delay

            $this->handleToolInvocations();
        };
    }

    private function validateChatRequest()
    {
        $request = $this->request;
        $validator = Validator::make($request->all(), [
            'messages' => 'required|array',
            'messages.*.role' => 'required|string|in:user,assistant,system',
            'messages.*.content' => 'required_without:messages.*.toolInvocations|nullable|string|max:25000',
            'messages.*.toolInvocations' => 'required_without:messages.*.content|nullable|array',
            'messages.*.toolInvocations.*.state' => 'required_with:messages.*.toolInvocations|string',
            'messages.*.toolInvocations.*.toolCallId' => 'required_with:messages.*.toolInvocations|string',
            'messages.*.toolInvocations.*.toolName' => 'required_with:messages.*.toolInvocations|string',
            'messages.*.toolInvocations.*.args' => 'required_with:messages.*.toolInvocations|array',
            'messages.*.toolInvocations.*.result' => 'required_with:messages.*.toolInvocations',
            // 'thread_id' => 'required|integer|exists:threads,id',
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
        // $thread_id = (int) $validatedData['thread_id'];
        $lastMessage = end($messages);

        $messageData = [
            'content' => $lastMessage['content'] ?? "(empty)",
            'thread_id' => $thread_id,
            'team_id' => null,
            // 'team_id' => $user->currentTeam ? $user->currentTeam->id : null,
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
        $assistantMessage = [
            'content' => $this->response['content'] ?? "(empty)",
            'thread_id' => 1, // $this->validatedData['thread_id'],
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
