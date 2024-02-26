<?php

namespace App\Services;

use App\AI\MistralAIGateway;
use App\Models\Conversation;
use OpenAI;

class Inferencer
{
    public static function llmInference($input, Conversation $conversation, $streamFunction)
    {
        $decodedInput = json_decode($input['input'], true);

        // Determine if input is multimodal (text + images)
        if (json_last_error() === JSON_ERROR_NONE && isset($decodedInput['images'])) {
            // It's JSON and has images, adjust handling for text and images
            $text = $decodedInput['text'] ?? '';
            $images = $decodedInput['images'] ?? [];

            // Assuming images are base64, we need to adjust how we pass this to OpenAI
            // For demonstration, let's assume a simplified scenario where we just pass the text and first image
            $inputForModel = [
                'text' => $text,
                // Assuming 'images' is an array of base64-encoded strings
                'image_url' => $images[0] ?? null, // Example: Taking the first image for simplicity
            ];

            $model = 'gpt-4-vision-preview';
            $messages = self::prepareMultiModalInference($inputForModel, $conversation);
            $client = OpenAI::client(env('OPENAI_API_KEY'));
        } else {
            // It's plain text or not properly decoded, proceed as before
            $text = $input['input'];
            $model = 'gpt-4';
            $messages = self::prepareTextInference($text, $conversation);
            $client = new MistralAIGateway();
            $inference = $client->inference($messages);
            $content = $inference['choices'][0]['message']['content'];
            return ['output' => $content];
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

    public static function llmInference_old($input, Conversation $conversation, $streamFunction)
    {
        $decodedInput = json_decode($input['input'], true);

        // Determine if input is multimodal (text + images)
        if (json_last_error() === JSON_ERROR_NONE && isset($decodedInput['images'])) {
            // It's JSON and has images, adjust handling for text and images
            $text = $decodedInput['text'] ?? '';
            $images = $decodedInput['images'] ?? [];

            // Assuming images are base64, we need to adjust how we pass this to OpenAI
            // For demonstration, let's assume a simplified scenario where we just pass the text and first image
            $inputForModel = [
                'text' => $text,
                // Assuming 'images' is an array of base64-encoded strings
                'image_url' => $images[0] ?? null, // Example: Taking the first image for simplicity
            ];

            $model = 'gpt-4-vision-preview';
            $messages = self::prepareMultiModalInference($inputForModel, $conversation);
        } else {
            // It's plain text or not properly decoded, proceed as before
            $text = $input['input'];
            $model = 'gpt-4';
            $messages = self::prepareTextInference($text, $conversation);
        }

        $client = OpenAI::client(env('OPENAI_API_KEY'));
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

    private static function prepareTextInference($text, Conversation $conversation)
    {
        $messages = [
            // System message
            [
                'role' => 'system',
                'content' => 'You are a helpful AI agent named '.$conversation->agent->name.'. Your description is '.$conversation->agent->description,
            ],
            // User message
            ['role' => 'user', 'content' => $text],
        ];

        return $messages;
    }

    private static function prepareMultiModalInference($input, Conversation $conversation)
    {
        // Initial text message from the system
        $systemMessage = [
            'role' => 'system',
            'content' => 'You are a helpful AI agent named '.$conversation->agent->name.'. Your description is '.$conversation->agent->description,
        ];

        // User message containing both text and image(s)
        $userMessageContent = [];

        // Add text part
        if (! empty($input['text'])) {
            $userMessageContent[] = [
                'type' => 'text',
                'text' => $input['text'],
            ];
        }

        // Add image part(s)
        if (! empty($input['image_url'])) {
            $base64prefixedPng = 'data:image/png;base64,'.$input['image_url'];
            // foreach ($input['input']['images'] as $imageUrl) {
            $userMessageContent[] = [
                'type' => 'image_url',
                // 'image_url' => 'https://upload.wikimedia.org/wikipedia/commons/thumb/d/dd/Gfp-wisconsin-madison-the-nature-boardwalk.jpg/2560px-Gfp-wisconsin-madison-the-nature-boardwalk.jpg', // $input['image_url'],
                'image_url' => $base64prefixedPng,
                // 'image_url' => $input['image_url'], // Ensure this is a string URL, not an array
            ];

        }

        $userMessage = [
            'role' => 'user',
            'content' => $userMessageContent, // This now directly matches the expected structure
        ];

        return [$systemMessage, $userMessage];
    }

    private static function old_prepareMultiModalInference($input, Conversation $conversation)
    {
        $messages = [
            // System message
            [
                'role' => 'system',
                'content' => [
                    'type' => 'text',
                    'text' => 'You are a helpful AI agent named '.$conversation->agent->name.'. Your description is '.$conversation->agent->description,
                ],
            ],
            // User message including both text and an image
            [
                'role' => 'user',
                'content' => [
                    ['type' => 'text', 'text' => $input['text']],
                    ['type' => 'image_url', 'image_url' => [
                        'url' => 'https://upload.wikimedia.org/wikipedia/commons/thumb/d/dd/Gfp-wisconsin-madison-the-nature-boardwalk.jpg/2560px-Gfp-wisconsin-madison-the-nature-boardwalk.jpg', // $input['image_url'],
                    ]],
                ],
            ],
        ];

        return $messages;
    }
}
