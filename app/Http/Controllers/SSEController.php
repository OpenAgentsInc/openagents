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
        Log::info('Received messages input', ['messages' => $messagesInput]);

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

            echo "data: " . json_encode(['type' => 'connection', 'content' => 'Connected']) . "\n\n";
            flush();
            Log::info('Sent connection event');

            $this->anthropicService->streamResponse($messages, function($data) {
                echo "data: " . json_encode($data) . "\n\n";
                flush();
                Log::info('Sent data to client', ['data' => $data]);
            });

            echo "data: " . json_encode(['type' => 'close', 'content' => 'Stream closed']) . "\n\n";
            flush();
            Log::info('Sent close event');
        }, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'Connection' => 'keep-alive',
            'X-Accel-Buffering' => 'no',
        ]);
    }

    private function parseMessages($input)
    {
        if (is_string($input)) {
            $decoded = json_decode($input, true);
            if (json_last_error() === JSON_ERROR_NONE && is_array($decoded)) {
                return $decoded;
            }
            return [['role' => 'user', 'content' => $input]];
        }
        if (is_array($input)) {
            return $input;
        }
        return [];
    }
}
