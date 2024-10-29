<?php

namespace App\Traits;

use Symfony\Component\HttpFoundation\StreamedResponse;
use Illuminate\Support\Facades\Log;

trait UsesStreaming
{
    protected function createStreamedResponse()
    {
        $request = $this->request;
        $callback = $this->callback;
        Log::info('UseChatController request', ['request' => $request->all()]);
        Log::info("Using model: {$this->model}");

        return $this->createStreamedResponseWithCallback($callback);
    }

    protected function createStreamedResponseWithCallback(callable $callback)
    {
        $response = new StreamedResponse($callback);

        $response->headers->set('Content-Type', 'text/event-stream');
        $response->headers->set('X-Accel-Buffering', 'no');
        $response->headers->set('Cache-Control', 'no-cache');

        return $response;
    }

    protected function stream($content)
    {
        $this->streamWithType('0', $content);
    }

    protected function streamContent($content)
    {
        $this->streamWithType('0', $content);
    }

    protected function streamToolCall(array $toolCalls)
    {
        Log::info('Streaming tool call', ['toolCalls' => $toolCalls]);
        foreach ($toolCalls as $toolCall) {
            if (
                !isset($toolCall['toolCallId']) || !is_string($toolCall['toolCallId']) ||
                !isset($toolCall['toolName']) || !is_string($toolCall['toolName']) ||
                !isset($toolCall['args']) || !is_array($toolCall['args'])
            ) {
                Log::warning('Invalid tool call format', ['toolCall' => $toolCall]);
                continue;
            }

            $this->streamWithType('9', [
                'toolCallId' => $toolCall['toolCallId'],
                'toolName' => $toolCall['toolName'],
                'args' => $toolCall['args'],
            ]);
        }
    }

    protected function streamToolResult(array $toolResult)
    {
        Log::info('Streaming tool result', ['toolResult' => $toolResult]);
        
        // Handle the nested structure
        if (isset($toolResult['toolResult'])) {
            $data = $toolResult['toolResult'];
            
            // Extract the tool call ID
            $toolCallId = $data['toolCallId'] ?? null;
            
            // Extract the actual result, which might be nested in result.value.result
            $result = null;
            if (isset($data['result']['value']['result'])) {
                $result = $data['result']['value']['result'];
            } elseif (isset($data['result']['value'])) {
                $result = $data['result']['value'];
            } elseif (isset($data['result'])) {
                $result = $data['result'];
            }

            if (!$toolCallId) {
                Log::warning('Invalid tool result format - missing toolCallId', ['toolResult' => $toolResult]);
                return;
            }

            $this->streamWithType('a', [
                'toolCallId' => $toolCallId,
                'result' => $result
            ]);
            return;
        }

        // Fallback for the original format
        if (!isset($toolResult['toolCallId']) || !is_string($toolResult['toolCallId'])) {
            Log::warning('Invalid tool result format - missing toolCallId', ['toolResult' => $toolResult]);
            return;
        }

        $this->streamWithType('a', [
            'toolCallId' => $toolResult['toolCallId'],
            'result' => $toolResult['result'] ?? null
        ]);
    }

    protected function streamFinishEvent($reason, $usage = null)
    {
        Log::info('Streaming finish event', ['reason' => $reason, 'usage' => $usage]);
        $data = [
            'finishReason' => $reason,
            'usage' => $usage ?? [
                'promptTokens' => $this->response['input_tokens'] ?? 0,
                'completionTokens' => $this->response['output_tokens'] ?? 0
            ]
        ];
        $this->streamWithType('d', $data);
    }

    private function streamWithType($type, $content)
    {
        $encodedContent = json_encode($content);
        echo "{$type}:{$encodedContent}\n";
        ob_flush();
        flush();
    }
}