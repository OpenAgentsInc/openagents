<?php

namespace App\Services;

use App\Models\Conversation;
use App\Models\Message;
use OpenAI;

class Inferencer
{
    public static function llmInference($input, Conversation $conversation, $streamFunction)
    {
        $messages = [
            [
                'role' => 'system',
                'content' => 'You are a helpful AI agent named '.$conversation->agent->name.'. Your description is '.$conversation->agent->description,
            ],
        ];

        $previousMessages = Message::where('conversation_id', $conversation->id)
            ->orderBy('created_at', 'asc')
            ->take(15)
            ->get()
            ->toArray();

        foreach ($previousMessages as $msg) {
            $messages[] = [
                'role' => $msg['sender'] === 'user' ? 'user' : 'assistant',
                'content' => $msg['body'],
            ];
        }

        // But shave off the most recent message (since we added it to DB already)
        array_pop($messages);

        // Add the current user input as the last element
        $messages[] = ['role' => 'user', 'content' => $input['input']];

        $client = OpenAI::client(env('OPENAI_API_KEY'));
        $stream = $client->chat()->createStreamed([
            'model' => 'gpt-4',
            'messages' => $messages,
            'max_tokens' => 3024,
        ]);

        $content = '';
        foreach ($stream as $response) {
            $token = $response['choices'][0]['delta']['content'] ?? '';
            $streamFunction($response);
            $content .= $token;
        }

        return [
            'output' => $content,
        ];
    }
}
