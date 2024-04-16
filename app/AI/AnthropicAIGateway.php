<?php
declare(strict_types=1);

namespace App\AI;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;

class AnthropicAIGateway implements GatewayInterface
{
    private Client $httpClient;

    public function __construct(Client $httpClient)
    {
        $this->httpClient = $httpClient;
    }

    public function inference(array $params): array
    {
        // If the role of the first message is 'system', remove that and set it as a separate variable
        $systemMessage = null;
        if ($params['messages'][0]['role'] === 'system') {
            $systemMessage = array_shift($params['messages'])['content'];
        }

        $data = [
            'model' => $params['model'],
            'messages' => $params['messages'],
            'max_tokens' => $params['max_tokens'],
            'stream' => true, // Ensure this is true for streaming
            'system' => $systemMessage ?? 'You are a helpful assistant.',
        ];

        // Add optional parameters if provided
        if (isset($params['temperature'])) {
            $data['temperature'] = $params['temperature'];
        }
        if (isset($params['top_p'])) {
            $data['top_p'] = $params['top_p'];
        }

        try {
            $response = $this->httpClient->post('https://api.anthropic.com/v1/messages', [
                'json' => $data,
                'stream' => true, // Important for handling streaming responses
                'headers' => [
                    'Content-Type' => 'application/json',
                    'x-api-key' => env('ANTHROPIC_API_KEY'), // Make sure to set your API key in your .env file
                    'anthropic-version' => '2023-06-01',
                    'anthropic-beta' => 'messages-2023-12-15',
                ],
            ]);

            $stream = $response->getBody();

            $content = '';
            $inputTokens = null;
            $outputTokens = null;

            foreach ($this->readStream($stream) as $event) {
                if ($event['type'] === 'content_block_delta' && isset($event['delta']['text'])) {
                    $content .= $event['delta']['text'];
                } elseif ($event['type'] === 'message_start' && isset($event['message']['usage']['input_tokens'])) {
                    $inputTokens = $event['message']['usage']['input_tokens'];
                } elseif ($event['type'] === 'message_delta' && isset($event['usage']['output_tokens'])) {
                    $outputTokens = $event['usage']['output_tokens'];
                }
            }

            return [
                'content' => $content,
                'input_tokens' => $inputTokens,
                'output_tokens' => $outputTokens,
            ];
        } catch (RequestException $e) {
            dd($e->getMessage());
        }
    }

    private function readStream($stream)
    {
        $buffer = '';
        while (! $stream->eof()) {
            $buffer .= $stream->read(1024);
            while (($pos = strpos($buffer, "\n")) !== false) {
                $line = substr($buffer, 0, $pos);
                $buffer = substr($buffer, $pos + 1);

                if (str_starts_with($line, 'data: ')) {
                    $data = json_decode(trim(substr($line, 5)), true);
                    if ($data) {
                        yield $data;
                    }
                }
            }
        }
    }
}
