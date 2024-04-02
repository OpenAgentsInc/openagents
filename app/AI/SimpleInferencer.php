<?php

namespace App\AI;

class SimpleInferencer
{
    public static function inference(string $prompt, string $model, callable $streamFunction): string
    {
        $messages = [
            [
                'role' => 'user',
                'content' => $prompt,
            ],
        ];

        // switch model
        switch ($model) {
            case 'mistral-large-latest':
                $client = new MistralAIGateway();
                break;
        }

        return $client->chat()->createStreamed([
            'model' => $model,
            'messages' => $messages,
            'max_tokens' => 9024,
            'stream_function' => $streamFunction,
        ]);
    }
}
