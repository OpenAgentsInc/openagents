<?php

namespace App\Services;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;
use Illuminate\Support\Facades\Log;

class AnthropicService
{
    protected $apiKey;
    protected $apiUrl = 'https://api.anthropic.com/v1/messages';

    public function __construct()
    {
        $this->apiKey = env('ANTHROPIC_API_KEY');
    }

public function streamResponse($messages, $systemPrompt, $codebases, $callback)
    {
        Log::info('Starting streamResponse', [
            'messageCount' => count($messages),
            'codebaseCount' => count($codebases),
            'systemPrompt' => $systemPrompt
        ]);

        $client = new Client();

        try {
            $response = $client->post($this->apiUrl, [
                'headers' => [
                    'Content-Type' => 'application/json',
                    'x-api-key' => $this->apiKey,
                    'anthropic-version' => '2023-06-01',
                ],
                'json' => [
                    'model' => 'claude-3-5-sonnet-20240620',
                    'max_tokens' => 1024,
                    'messages' => $messages,
                    'stream' => true,
                    'system' => $systemPrompt,
                    'tool_choice' => ['type' => 'auto'],
                    'tools' => [
                        [
                            'name' => 'search_codebase',
                            'description' => 'Search for code snippets or information within the specified codebases. This tool allows you to find relevant code, documentation, or comments that might help answer the user\'s query. Use this when you need to reference specific code or gather information from the project repositories.',
                            'input_schema' => [
                                'type' => 'object',
                                'properties' => [
                                    'query' => [
                                        'type' => 'string',
                                        'description' => 'The natural language search query to find the right code in the repo'
                                    ]
                                ],
                                'required' => ['query']
                            ]
                        ]
                    ]
                ],
                'stream' => true,
            ]);

            Log::info('Anthropic API request sent', ['url' => $this->apiUrl]);

            $body = $response->getBody();
            $buffer = '';

            while (!$body->eof()) {
                $chunk = $body->read(1024);
                $buffer .= $chunk;

                $lines = explode("\n", $buffer);
                $buffer = array_pop($lines);  // Keep the last incomplete line in the buffer

                foreach ($lines as $line) {
                    $this->processLine($line, $callback);
                }
            }

            // Process any remaining data in the buffer
            if (!empty($buffer)) {
                $this->processLine($buffer, $callback);
            }

            Log::info('Finished streaming response');
        } catch (RequestException $e) {
            Log::error('Error in streamResponse', [
                'error' => $e->getMessage(),
                'response' => $e->hasResponse() ? $e->getResponse()->getBody()->getContents() : null
            ]);
            throw $e;
        }
    }

private function processLine($line, $callback)
    {
        if (str_starts_with($line, 'data: ')) {
            $data = json_decode(substr($line, 6), true);
            Log::debug('Received data from Anthropic API', ['data' => $data]);

            if ($data === null) {
                Log::info('Received null data (likely a ping)');
                return;
            }

            switch ($data['type']) {
                case 'content_block_delta':
                    if (isset($data['delta']['text'])) {
                        Log::info('Received content block delta', ['text' => $data['delta']['text']]);
                        $callback([
                            'type' => 'token',
                            'content' => $data['delta']['text'],
                        ]);
                    }
                    break;
                case 'tool_use':
                    Log::info('Received tool use request', ['data' => $data]);
                    $callback([
                        'type' => 'tool_use',
                        'content' => json_encode($data),
                    ]);
                    break;
                case 'message_start':
                case 'content_block_start':
                case 'content_block_stop':
                case 'message_delta':
                case 'message_stop':
                    $callback([
                        'type' => $data['type'],
                        'content' => json_encode($data),
                    ]);
                    break;
                case 'ping':
                    Log::info('Received ping');
                    break;
                default:
                    Log::warning('Received unknown event type', ['data' => $data]);
            }
        }
    }
}
