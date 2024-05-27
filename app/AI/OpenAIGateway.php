<?php

declare(strict_types=1);

namespace App\AI;

use OpenAI;

class OpenAIGateway implements GatewayInterface
{
    private object $client;

    public function __construct(?object $client = null)
    {
        $this->client = $client ?? OpenAI::client(env('OPENAI_API_KEY'));
    }

    public function models()
    {
        $response = $this->client->models()->list();
        $ids = [];
        foreach ($response->data as $result) {
            $ids[] = $result->id;
        }
        dd($ids);
    }

    public function inference(array $params): array
    {
        $model = $params['model'];
        $messages = $params['messages'];
        $streamFunction = $params['stream_function'];

        $message = '';

        $stream = $this->client->chat()->createStreamed([
            'model' => $model,
            'messages' => $messages,
            'stream_options' => [
                'include_usage' => true,
            ],
        ]);

        $inputTokens = $outputTokens = 0;
        foreach ($stream as $response) {
            $content = $response['choices'][0]['delta']['content'] ?? '';
            $streamFunction($content);
            $message .= $content;
            if (isset($response['usage'])) {
                $inputTokens = $response['usage']['prompt_tokens'] ?? $inputTokens;
                $outputTokens = $response['usage']['completion_tokens'] ?? $outputTokens;
            }
        }

        return [
            'content' => $message,
            'input_tokens' => $inputTokens,
            'output_tokens' => $outputTokens,
        ];
    }
}
