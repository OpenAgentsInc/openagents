<?php

namespace App\Services;

use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Log;
use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;

class AnthropicService
{
    private $apiKey;
    private $baseUrl = 'https://api.anthropic.com/v1';
    private $model = 'claude-3-5-sonnet-20240620';

    public function __construct()
    {
        $this->apiKey = config('services.anthropic.api_key');
    }

    public function streamResponse($userMessage, $callback)
    {
        $url = $this->baseUrl . '/messages';
        $data = [
            'model' => $this->model,
            'max_tokens' => 1024,
            'messages' => [
                ['role' => 'user', 'content' => $userMessage]
            ],
            'stream' => true
        ];

        Log::info('Sending request to Anthropic API', ['url' => $url, 'data' => $data]);

        try {
            $client = new Client();
            $response = $client->post($url, [
                'headers' => [
                    'x-api-key' => $this->apiKey,
                    'anthropic-version' => '2023-06-01',
                    'content-type' => 'application/json',
                ],
                'json' => $data,
                'stream' => true,
            ]);

            $body = $response->getBody();
            $buffer = '';

            while (!$body->eof()) {
                $chunk = $body->read(1024);
                Log::info('Received chunk from Anthropic API', ['chunk' => $chunk]);
                $buffer .= $chunk;
                $events = explode("\n\n", $buffer);

                foreach ($events as $i => $event) {
                    if ($i === count($events) - 1) {
                        $buffer = $event;
                        break;
                    }

                    $lines = explode("\n", $event);
                    $eventType = null;
                    $eventData = null;

                    foreach ($lines as $line) {
                        if (str_starts_with($line, 'event: ')) {
                            $eventType = trim(substr($line, 7));
                        } elseif (str_starts_with($line, 'data: ')) {
                            $eventData = json_decode(trim(substr($line, 6)), true);
                        }
                    }

                    if ($eventType && $eventData) {
                        $this->processEvent($eventType, $eventData, $callback);
                    }
                }
            }
        } catch (RequestException $e) {
            Log::error('Error in Anthropic API request: ' . $e->getMessage());
            Log::error($e->getTraceAsString());
            throw $e;
        }
    }

    private function processEvent($eventType, $eventData, $callback)
    {
        Log::info('Processing event', ['type' => $eventType, 'data' => $eventData]);

        switch ($eventType) {
            case 'content_block_start':
                // Ignore this event as it doesn't contain any content
                break;
            case 'content_block_delta':
                if (isset($eventData['delta']['text'])) {
                    $callback(['type' => 'token', 'content' => $eventData['delta']['text']]);
                }
                break;
            case 'message_stop':
                $callback(['type' => 'end']);
                break;
            case 'ping':
                // Ignore ping events
                break;
            default:
                // Log other event types for debugging
                Log::info('Unhandled event type', ['type' => $eventType, 'data' => $eventData]);
                break;
        }
    }
}
