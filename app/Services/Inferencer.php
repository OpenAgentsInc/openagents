<?php

namespace App\Services;

use App\Models\Conversation;
use App\Models\Message;
use OpenAI;

class Inferencer
{
    public static function llmInference($input, Conversation $conversation, $streamFunction)
    {
        $input = [
            'input' => [
                'text' => $input['input'],
                'image_url' => 'https://private-user-images.githubusercontent.com/14167547/307157769-a949dbcb-afa7-4e0b-b341-3e9625f304fb.png?jwt=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJnaXRodWIuY29tIiwiYXVkIjoicmF3LmdpdGh1YnVzZXJjb250ZW50LmNvbSIsImtleSI6ImtleTUiLCJleHAiOjE3MDg3MDMzNTEsIm5iZiI6MTcwODcwMzA1MSwicGF0aCI6Ii8xNDE2NzU0Ny8zMDcxNTc3NjktYTk0OWRiY2ItYWZhNy00ZTBiLWIzNDEtM2U5NjI1ZjMwNGZiLnBuZz9YLUFtei1BbGdvcml0aG09QVdTNC1ITUFDLVNIQTI1NiZYLUFtei1DcmVkZW50aWFsPUFLSUFWQ09EWUxTQTUzUFFLNFpBJTJGMjAyNDAyMjMlMkZ1cy1lYXN0LTElMkZzMyUyRmF3czRfcmVxdWVzdCZYLUFtei1EYXRlPTIwMjQwMjIzVDE1NDQxMVomWC1BbXotRXhwaXJlcz0zMDAmWC1BbXotU2lnbmF0dXJlPTA3YzBiM2E5ZDVhMGNhNmY2ZjYzNzRiZTZkMDMxZGUwYWU1YTkzZDViMjkzNWI2MTU1NWFjYTgyOGFlODg4MzcmWC1BbXotU2lnbmVkSGVhZGVycz1ob3N0JmFjdG9yX2lkPTAma2V5X2lkPTAmcmVwb19pZD0wIn0.fdl_fDZCP3cBnqMwkyUGKzH9tlNlrWmzkFyP5yTDouQ',
            ],
        ];
        dd($input);
        $client = OpenAI::client(env('OPENAI_API_KEY'));

        if (gettype($input['input']) === 'string') {
            $model = 'gpt-4';
            $messages = self::prepareTextInference($input, $conversation);
        } else {
            //Handle multimodal input
            $model = 'gpt-4-vision-preview';
            $messages = self::prepareMultiModalInference($input, $conversation);
        }

        $stream = $client->chat()->createStreamed([
            'model' => $model,
            'messages' => $messages,
            'max_tokens' => 3024,
        ]);

        $content = '';
        foreach ($stream as $response) {
            $token = $response['choices'][0]['delta']['content'] ?? '';
            $streamFunction($response);
            $content .= $token;
        }

        return ['output' => $content];
    }

    private static function prepareTextInference($input, Conversation $conversation)
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

        return $messages;
    }

    private static function prepareMultiModalInference($input, Conversation $conversation)
    {
        return [
            ['role' => 'system',
                'content' => [
                    'type' => 'text',
                    'text' => 'You are a helpful AI agent named '.$conversation->agent->name.'. Your description is '.$conversation->agent->description,
                ]],
            ['role' => 'user',
                'content' => [
                    'type' => 'text',
                    'text' => $input['input']['text'],
                ],
            ],
            ['role' => 'user',
                'content' => [
                    'type' => 'image_url',
                    'image_url' => [
                        'url' => $input['input']['image_url'],
                    ],
                ],
            ],
        ];
    }
}
