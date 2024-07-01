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
        $codebasesInput = $request->input('codebases');
        Log::info('Received raw messages input', ['rawMessages' => $messagesInput]);
        Log::info('Received codebases input', ['codebases' => $codebasesInput]);

        $messages = $this->parseMessages($messagesInput);
        $codebases = $this->parseCodebases($codebasesInput);

        if (empty($messages)) {
            Log::error('Invalid or empty message format', ['input' => $messagesInput]);
            return Response::json(['error' => 'Invalid or empty message format'], 400);
        }

        Log::info('Parsed messages', ['messages' => $messages]);
        Log::info('Parsed codebases', ['codebases' => $codebases]);

        $systemPrompt = $this->buildSystemPrompt($codebases);

        return Response::stream(function() use ($messages, $systemPrompt) {
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
                $this->anthropicService->streamResponse($messages, $systemPrompt, function($data) {
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

    private function parseCodebases($input)
    {
        Log::info('Parsing codebases input', ['input' => $input]);
        if (is_string($input)) {
            $decoded = json_decode($input, true);
            if (json_last_error() === JSON_ERROR_NONE && is_array($decoded)) {
                Log::info('Decoded JSON string to array', ['decoded' => $decoded]);
                return $decoded;
            }
        }
        if (is_array($input)) {
            Log::info('Input is already an array', ['array' => $input]);
            return $input;
        }
        Log::warning('Input is neither string nor array, returning empty array');
        return [];
    }

    private function buildSystemPrompt($codebases)
    {
        $basePrompt = "You are a coding agent named AutoDev created by OpenAgents. You help the user create and execute plans to assist their coding goals. When asked to create a plan, you write it in Markdown and put it in tags <plan> and </plan> so it can be displayed separately to the user.";

        if (!empty($codebases)) {
            $codebaseList = implode(", ", array_map(function($codebase) {
                return "{$codebase['name']} (branch: {$codebase['branch']})";
            }, $codebases));

            $basePrompt .= " You have the ability to search the following codebases before responding to the user query: $codebaseList. If you decide to search these codebases, reply with a JSON blob for function calling that we'll use to call the Greptile API.";
        }

        return $basePrompt;
    }
}
