<?php

declare(strict_types=1);

namespace App\AI;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;
use Illuminate\Support\Facades\Http;

class MistralAIGateway implements GatewayInterface
{
    private Client $httpClient;

    public function __construct(Client $httpClient)
    {
        $this->httpClient = $httpClient;
    }

    public function models()
    {
        $url = 'https://api.mistral.ai/v1/models';
        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
            'Authorization' => 'Bearer '.env('MISTRAL_API_KEY'),
        ])->get($url);

        dd($response->json());
    }

    public function embed($inputs)
    {
        // Your API endpoint for embeddings
        $url = 'https://api.mistral.ai/v1/embeddings';

        // Prepare the data payload
        $data = [
            'model' => 'mistral-embed', // Assuming 'mistral-embed' is the correct model ID for embeddings
            'input' => $inputs,
            'encoding_format' => 'float', // Assuming you want the output in float format
        ];

        // Make the HTTP POST request
        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
            'Authorization' => 'Bearer '.env('MISTRAL_API_KEY'),
        ])->post($url, $data);

        // Check if the request was successful
        if ($response->successful()) {
            // Return the response body
            $json = $response->json();

            return $json['data'][0]['embedding'];
        } else {
            // Handle the error
            return [
                'error' => 'Failed to retrieve embeddings',
                'details' => $response->json(),
            ];
        }
    }

    public function inference(array $params): array
    {
        $url = 'https://api.mistral.ai/v1/chat/completions';
        $apiKey = env('MISTRAL_API_KEY');
        $model = $params['model'];
        $messages = $params['messages'];
        $maxTokens = $params['max_tokens'];
        $streamFunction = $params['stream_function'];
        $temperature = $params['temperature'] ?? 0.7;
        $topP = $params['top_p'] ?? 1;

        $data = [
            'model' => $model,
            'messages' => $messages,
            'max_tokens' => $maxTokens,
            'temperature' => $temperature,
            'top_p' => $topP,
            'stream' => true, // Ensure this is true for streaming
        ];

        try {
            $response = $this->httpClient->post($url, [
                'json' => $data,
                'stream' => true, // Important for streaming
                'headers' => [
                    'Authorization' => 'Bearer '.$apiKey,
                ],
            ]);

            $stream = $response->getBody();
            $content = '';
            $inputTokens = 0;
            $outputTokens = 0;

            foreach ($this->readStream($stream) as $chunk) {
                $content = $chunk['content'];
                $inputTokens = $chunk['input_tokens'];
                $outputTokens += $chunk['output_tokens'];

                // Call the stream function with the updated content
                $streamFunction($content);
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
        $content = '';
        $inputTokens = null;
        $outputTokens = null;

        while (! $stream->eof()) {
            $line = $this->readLine($stream);

            if (! str_starts_with($line, 'data:')) {
                continue;
            }

            $data = trim(substr($line, 5)); // Skip the 'data:' part
            if ($data === '[DONE]') {
                break;
            }

            $response = json_decode($data, true);
            if ($response) {
                if (isset($response['choices'][0]['delta']['content'])) {
                    $content .= $response['choices'][0]['delta']['content'];
                }

                if (isset($response['usage'])) {
                    $inputTokens = $response['usage']['prompt_tokens'];
                    $outputTokens = $response['usage']['completion_tokens'];
                }

                yield [
                    'content' => $content,
                    'input_tokens' => $inputTokens,
                    'output_tokens' => $outputTokens,
                ];
            }
        }
    }

    private function readLine($stream)
    {
        $line = '';
        while (! $stream->eof()) {
            $char = $stream->read(1);
            if ($char === "\n") {
                break;
            }
            $line .= $char;
        }

        return $line;
    }
}
