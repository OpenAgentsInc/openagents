<?php

namespace App\Services;

use App\Models\Conversation;
use App\Models\Message;
use OpenAI;

class Inferencer
{
    public static function llmInference($input, Conversation $conversation, $streamFunction)
    {
        $decodedInput = json_decode($input['input'], true);
        if (json_last_error() === JSON_ERROR_NONE) {
            // It's JSON, adjust handling for text and images
            $text = $decodedInput['text'] ?? '';
            $images = $decodedInput['images'] ?? [];
            // Modify how you construct the input for the model here
            // Possibly include both text and images
        } else {
            // It's plain text, proceed as before
            $text = $input['input'];
            $images = []; // No images
        }

        // $input = [
        //     'input' => [
        //         'text' => $input['input'],
        //         'image_url' => 'https://upload.wikimedia.org/wikipedia/commons/thumb/d/dd/Gfp-wisconsin-madison-the-nature-boardwalk.jpg/2560px-Gfp-wisconsin-madison-the-nature-boardwalk.jpg',
        //     ],
        // ];
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
        $messages = [
            [
                'role' => 'system',
                'content' => [
                    [
                        'type' => 'text',
                        'text' => 'You are a helpful AI agent named '.$conversation->agent->name.'. Your description is '.$conversation->agent->description,
                    ],
                ],
            ],
            [
                'role' => 'user',
                'content' => [
                    [
                        'type' => 'text',
                        'text' => $input['input']['text'],
                    ],
                    [
                        'type' => 'image_url',
                        'image_url' => $input['input']['image_url'], // Directly use the provided image URL
                    ],
                ],
            ],
        ];

        return $messages;
    }
}
