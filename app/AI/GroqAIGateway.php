<?php

namespace App\AI;

use GuzzleHttp\Client;
use GuzzleHttp\Exception\RequestException;
use Illuminate\Support\Facades\Http;

class GroqAIGateway
{
    private $url = 'https://api.groq.com/openai/v1/chat/completions';

    public function chat()
    {
        return new GroqAIChat();
    }

    public function inference($messages)
    {
        // Prepare the data payload
        $data = [
            'model' => 'mixtral-8x7b-32768',
            'messages' => $messages,
            'max_tokens' => 2000,
            'temperature' => 0.5,
            'top_p' => 1,
        ];

        // Make the HTTP POST request
        $response = Http::withHeaders([
            'Content-Type' => 'application/json',
            'Authorization' => 'Bearer '.env('GROQ_API_KEY'),
        ])->post($this->url, $data);

        // Check if the request was successful
        if ($response->successful()) {
            // Return the response body
            return $response->json();
        } else {
            // Handle the error (you can customize this part based on your needs)
            return [
                'error' => 'Failed to make inference',
                'details' => $response->json(),
            ];
        }
    }
}

class GroqAIChat
{
    private $url = 'https://api.groq.com/openai/v1/chat/completions';

    public function createStreamed($params)
    {
        $apiKey = env('GROQ_API_KEY');
        $model = $params['model'];
        $messages = $params['messages'];
        $maxTokens = $params['max_tokens'];
        $streamFunction = $params['stream_function'];
        $temperature = $params['temperature'] ?? 0.5;
        $topP = $params['top_p'] ?? 1;

        $client = new Client();

        $data = [
            'model' => $model,
            'messages' => $messages,
            'max_tokens' => $maxTokens,
            'temperature' => $temperature,
            'top_p' => $topP,
            'stream' => true, // Ensure this is true for streaming
        ];

        try {
            $response = $client->post($this->url, [
                'json' => $data,
                'stream' => true, // Important for streaming
                'headers' => [
                    'Authorization' => 'Bearer '.$apiKey,
                ],
            ]);

            $stream = $response->getBody();
            $content = '';
            foreach ($this->readStream($stream) as $responseLine) {
                if (isset($responseLine['choices'][0]['delta']['content'])) {
                    $content .= $responseLine['choices'][0]['delta']['content'];
                    $streamFunction($responseLine);
                    // Here, you could also broadcast or handle each token/message part as it arrives
                }
            }

            return $content;
        } catch (RequestException $e) {
            // Handle exception or error
            dd('error', $e->getMessage());

            return 'Error: '.$e->getMessage();
        }
    }

    private function readStream($stream)
    {
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
                yield $response;
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
