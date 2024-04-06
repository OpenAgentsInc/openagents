<?php

namespace App\AI;

use OpenAI;

class OpenAIGateway
{
    public function __construct()
    {
        $this->client = OpenAI::client(env('OPENAI_API_KEY'));
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

    public function stream($params)
    {
        $model = $params['model'];
        $messages = $params['messages'];
        $max_tokens = $params['max_tokens'];
        $stream_function = $params['stream_function'];

        $message = '';

        $stream = $this->client->chat()->createStreamed([
            'model' => $model,
            'messages' => $messages,
            'max_tokens' => $max_tokens,
        ]);

        foreach ($stream as $response) {
            $stream_function($response);
            $message .= $response['choices'][0]['delta']['content'] ?? '';
        }

        return $message;
    }
}
