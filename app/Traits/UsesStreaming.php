<?php

namespace App\Traits;

use Symfony\Component\HttpFoundation\StreamedResponse;
use Illuminate\Http\Request;
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
        if (
            !isset($toolResult['toolCallId']) || !is_string($toolResult['toolCallId']) ||
            !isset($toolResult['result'])
        ) {
            Log::warning('Invalid tool result format', ['toolResult' => $toolResult]);
            return;
        }

        $this->streamWithType('a', [
            'toolCallId' => $toolResult['toolCallId'],
            'result' => $toolResult['result'],
        ]);
    }

    private function streamWithType($type, $content)
    {
        $encodedContent = json_encode($content);
        echo "{$type}:{$encodedContent}\n";
        ob_flush();
        flush();
    }
}