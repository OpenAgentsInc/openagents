<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Support\Facades\Response;
use App\Services\AnthropicService;
use Illuminate\Support\Facades\Log;

class SSEController extends Controller
{
    protected $anthropicService;

    public function __construct(AnthropicService $anthropicService)
    {
        $this->anthropicService = $anthropicService;
    }

    public function stream(Request $request)
    {
        $messagesInput = $request->input('messages');
        Log::info('Received raw messages input', ['rawMessages' => $messagesInput]);

        $messages = $this->parseMessages($messagesInput);

        if (empty($messages)) {
            Log::error('Invalid or empty message format', ['input' => $messagesInput]);
            return Response::json(['error' => 'Invalid or empty message format'], 400);
        }

        Log::info('Parsed messages', ['messages' => $messages]);

        return Response::stream(function() use ($messages) {
            Log::info('Starting SSE stream', ['messageCount' => count($messages)]);

            if (ob_get_level() > 0) {
                ob_end_clean();
            }

            header('Content-Type: text/event-stream');
            header('Cache-Control: no-cache');
            header('Connection: keep-alive');
            header('X-Accel-Buffering: no');

            $this->sendEvent('connection', 'Connected');

            try {
                $this->anthropicService->streamResponse($messages, function($data) {
                    $this->sendEvent($data['type'], $data['content']);
                });
            } catch (\Exception $e) {
                Log::error('Error in streamResponse', ['error' => $e->getMessage()]);
                $this->sendEvent('error', 'An error occurred while processing your request');
            }

            $this->sendEvent('close', 'Stream closed');

        }, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'Connection' => 'keep-alive',
            'X-Accel-Buffering' => 'no',
        ]);
    }

    private function sendEvent($type, $content)
    {
        $data = json_encode(['type' => $type, 'content' => $content]);
        echo "id: " . microtime(true) . "\n";
        echo "event: message\n";
        echo "data: " . $data . "\n\n";
        flush();
        if (ob_get_level() > 0) {
            ob_flush();
        }
        Log::info('Sent event', ['type' => $type, 'content' => $content]);
    }

    private function parseMessages($input)
    {
        Log::info('Parsing messages input', ['input' => $input]);
        if (is_string($input)) {
            $decoded = json_decode($input, true);
            if (json_last_error() === JSON_ERROR_NONE && is_array($decoded)) {
                Log::info('Decoded JSON string to array', ['decoded' => $decoded]);
                return $decoded;
            }
            Log::info('Treating input as single user message', ['message' => $input]);
            return [['role' => 'user', 'content' => $input]];
        }
        if (is_array($input)) {
            Log::info('Input is already an array', ['array' => $input]);
            return $input;
        }
        Log::warning('Input is neither string nor array, returning empty array');
        return [];
    }
}
