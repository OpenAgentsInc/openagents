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
    private $toolUseData = null;
    private $toolUseInput = '';

    public function __construct(AnthropicService $anthropicService, GreptileService $greptileService)
    {
        $this->anthropicService = $anthropicService;
        $this->greptileService = $greptileService;
    }

    public function stream(Request $request)
    {
        $messagesInput = $request->input('messages');
        $codebasesInput = $request->input('codebases');
        Log::info('Received request', [
            'rawMessages' => $messagesInput,
            'codebases' => $codebasesInput
        ]);

        $messages = $this->parseMessages($messagesInput);
        $codebases = $this->parseCodebases($codebasesInput);

        if (empty($messages)) {
            Log::error('Invalid or empty message format', ['input' => $messagesInput]);
            return Response::json(['error' => 'Invalid or empty message format'], 400);
        }

        Log::info('Parsed input', [
            'messages' => $messages,
            'codebases' => $codebases
        ]);

        $systemPrompt = $this->buildSystemPrompt($codebases);
        Log::info('Built system prompt', ['systemPrompt' => $systemPrompt]);

        return Response::stream(function () use ($messages, $systemPrompt, $codebases) {
            Log::info('Starting SSE stream', [
                'messageCount' => count($messages),
                'codebaseCount' => count($codebases)
            ]);

            if (ob_get_level() > 0) {
                ob_end_clean();
            }

            header('Content-Type: text/event-stream');
            header('Cache-Control: no-cache');
            header('Connection: keep-alive');
            header('X-Accel-Buffering: no');

            $this->sendEvent('connection', 'Connected');

            $toolUseData = null;
            $toolUseInput = '';

            $this->anthropicService->streamResponse($messages, $systemPrompt, $codebases, function ($data) use ($codebases, &$toolUseData, &$toolUseInput) {
                Log::info('Received data from AnthropicService', ['data' => $data]);

                if ($data['type'] === 'content_block_start' && strpos($data['content'], '"type":"tool_use"') !== false) {
                    $contentData = json_decode($data['content'], true);
                    $toolUseData = $contentData['content_block'];
                    $toolUseInput = '';
                    Log::info('Started receiving tool use data', ['toolUseData' => $toolUseData]);
                } elseif ($data['type'] === 'tool_use_input') {
                    $toolUseInput .= $data['content'];
                    Log::info('Accumulating tool use input', ['toolUseInput' => $toolUseInput]);
                } elseif ($data['type'] === 'content_block_stop' && $toolUseData !== null) {
                    Log::info('Attempting to process tool use', ['toolUseData' => $toolUseData, 'toolUseInput' => $toolUseInput]);
                    try {
                        $parsedInput = json_decode($toolUseInput, true);
                        if (json_last_error() !== JSON_ERROR_NONE) {
                            throw new \Exception('Invalid JSON in tool use input: ' . json_last_error_msg());
                        }
                        $toolUseData['input'] = $parsedInput;
                        Log::info('Parsed tool use input', ['parsedInput' => $parsedInput]);
                        $searchResult = $this->handleToolUse($toolUseData, $codebases);
                        Log::info('Search result received', ['searchResult' => $searchResult]);
                        $this->sendEvent('tool_result', json_encode($searchResult));

                        // Add the tool result to the chat history
                        $this->sendEvent('chat_history', json_encode([
                            'role' => 'system',
                            'content' => "Tool result: " . json_encode($searchResult)
                        ]));
                    } catch (\Exception $e) {
                        Log::error('Error handling tool use', ['error' => $e->getMessage(), 'toolUseData' => $toolUseData, 'toolUseInput' => $toolUseInput]);
                    }
                    $toolUseData = null;
                    $toolUseInput = '';
                } else {
                    $this->sendEvent($data['type'], $data['content']);
                }
            });

            $this->sendEvent('close', 'Stream closed');
        }, 200, [
            'Cache-Control' => 'no-cache',
            'Content-Type' => 'text/event-stream',
            'Connection' => 'keep-alive',
            'X-Accel-Buffering' => 'no',
        ]);
    }

    private function parseContentData($data)
    {
        if (isset($data['content']) && is_string($data['content'])) {
            $decodedContent = json_decode($data['content'], true);
            if (json_last_error() === JSON_ERROR_NONE) {
                return isset($decodedContent['content_block']) ? $decodedContent['content_block'] : $decodedContent;
            }
        }
        return $data['content'] ?? [];
    }

    private function processToolUse($codebases)
    {
        Log::info('Completed receiving tool use data', ['toolUseInput' => $this->toolUseInput]);
        try {
            $this->toolUseData['input'] = json_decode($this->toolUseInput, true);
            Log::info('Parsed tool use input', ['parsedInput' => $this->toolUseData['input']]);
            $searchResult = $this->handleToolUse($this->toolUseData, $codebases);
            Log::info('Search result received', ['searchResult' => $searchResult]);
            $this->sendEvent('tool_result', json_encode($searchResult));
        } catch (\Exception $e) {
            Log::error('Error handling tool use', ['error' => $e->getMessage(), 'toolUseData' => $this->toolUseData, 'toolUseInput' => $this->toolUseInput]);
        }
        $this->toolUseData = null;
        $this->toolUseInput = '';
    }

    private function logDataDetails($data)
    {
        Log::info('Data type', ['type' => $data['type']]);
        if (isset($data['content'])) {
            Log::info('Data content', ['content' => $data['content']]);
        }
        if (isset($data['content_block'])) {
            Log::info('Content block', ['content_block' => $data['content_block']]);
        } elseif (isset($data['content']['content_block'])) {
            Log::info('Content block', ['content_block' => $data['content']['content_block']]);
        } else {
            Log::info('No content block in data');
        }
    }

    private function handleToolUse($toolUseData, $codebases)
    {
        Log::info('handleToolUse method called', ['toolUseData' => $toolUseData, 'codebases' => $codebases]);

        if ($toolUseData['name'] === 'search_codebase') {
            $query = $toolUseData['input']['query'];

            Log::info('Searching codebase', ['query' => $query]);

            // Format repositories for Greptile API
            $repositories = array_map(function ($codebase) {
                return [
                    'remote' => 'github',
                    'repository' => $codebase['name'],
                    'branch' => $codebase['branch']
                ];
            }, $codebases);

            Log::info('Formatted repositories for Greptile', ['repositories' => $repositories]);

            // Perform the search using the GreptileService
            $searchResult = $this->greptileService->searchCodebase($query, $repositories);

            Log::info('Greptile search result received', ['resultSize' => strlen(json_encode($searchResult))]);

            // Send the Greptile response as a separate event
            $this->sendEvent('greptile_result', json_encode([
                'tool_use_id' => $toolUseData['id'],
                'content' => $searchResult
            ]));

            return [
                'tool_use_id' => $toolUseData['id'],
                'content' => $searchResult
            ];
        }

        Log::warning('Unknown tool', ['toolName' => $toolUseData['name']]);
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
            $codebaseList = implode(", ", array_map(function ($codebase) {
                return "{$codebase['name']} (branch: {$codebase['branch']})";
            }, $codebases));

            $basePrompt .= " You have the ability to search the following codebases before responding to the user query: $codebaseList. If you decide to search these codebases, use the search_codebase tool.";
        }

        return $basePrompt;
    }
}
