<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Support\Facades\Response;
use App\Services\AnthropicService;
use App\Services\GreptileService;
use Illuminate\Support\Facades\Log;

class SSEController extends Controller
{
    protected $anthropicService;
    protected $greptileService;

    public function __construct(AnthropicService $anthropicService, GreptileService $greptileService)
    {
        $this->anthropicService = $anthropicService;
        $this->greptileService = $greptileService;
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

        return Response::stream(function() use ($messages, $systemPrompt, $codebases) {
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
                $this->anthropicService->streamResponse($messages, $systemPrompt, $codebases, function($data) use ($codebases) {
                    if ($data['type'] === 'tool_use') {
                        $toolUseData = json_decode($data['content'], true);
                        $searchResult = $this->handleToolUse($toolUseData, $codebases);
                        $this->sendEvent('tool_result', json_encode($searchResult));
                    } else {
                        $this->sendEvent($data['type'], $data['content']);
                    }
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

    private function handleToolUse($toolUseData, $codebases)
    {
        if ($toolUseData['name'] === 'search_codebase') {
            $input = $toolUseData['input'];
            $codebase = $input['codebase'];
            $branch = $input['branch'];
            $query = $input['query'];

            // Verify that the requested codebase is in the list of allowed codebases
            $allowedCodebase = collect($codebases)->first(function ($c) use ($codebase, $branch) {
                return $c['name'] === $codebase && $c['branch'] === $branch;
            });

            if (!$allowedCodebase) {
                return [
                    'error' => 'Requested codebase or branch is not allowed',
                    'tool_use_id' => $toolUseData['id']
                ];
            }

            // Perform the search using the GreptileService
            $searchResult = $this->greptileService->searchCodebase($codebase, $branch, $query);

            return [
                'tool_use_id' => $toolUseData['id'],
                'content' => $searchResult
            ];
        }

        return [
            'error' => 'Unknown tool',
            'tool_use_id' => $toolUseData['id']
        ];
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
