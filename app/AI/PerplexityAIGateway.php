<?php
declare(strict_types=1);

namespace App\AI;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;

class PerplexityAIGateway implements GatewayInterface
{
    private Client $httpClient;

    public function __construct(Client $httpClient)
    {
        $this->httpClient = $httpClient;
    }

    public function inference(array $params): array
    {
        $data = [
            'model' => $params['model'],
            'messages' => $params['messages'],
            'stream' => true, // Ensure this is true for streaming
        ];

        // Add optional parameters if provided
        if (isset($params['max_tokens'])) {
            $data['max_tokens'] = $params['max_tokens'];
        }
        if (isset($params['temperature'])) {
            $data['temperature'] = $params['temperature'];
        }
        if (isset($params['top_p'])) {
            $data['top_p'] = $params['top_p'];
        }
        if (isset($params['top_k'])) {
            $data['top_k'] = $params['top_k'];
        }
        if (isset($params['presence_penalty'])) {
            $data['presence_penalty'] = $params['presence_penalty'];
        }
        if (isset($params['frequency_penalty'])) {
            $data['frequency_penalty'] = $params['frequency_penalty'];
        }

        try {
            $response = $this->httpClient->request('POST', 'https://api.perplexity.ai/chat/completions', [
                'body' => json_encode($data),
                'headers' => [
                    'accept' => 'application/json',
                    'Authorization' => 'Bearer '.env('PERPLEXITY_API_KEY'),
                    'content-type' => 'application/json',
                ],
            ]);

            $stream = $response->getBody();

            $content = '';
            $inputTokens = null;
            $outputTokens = null;

            foreach ($this->readStream($stream) as $event) {
                if (isset($event['choices'][0]['delta']['content'])) {
                    $content .= $event['choices'][0]['delta']['content'];
                }
                if (isset($event['usage']['prompt_tokens'])) {
                    $inputTokens = $event['usage']['prompt_tokens'];
                }
                if (isset($event['usage']['completion_tokens'])) {
                    $outputTokens = $event['usage']['completion_tokens'];
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
