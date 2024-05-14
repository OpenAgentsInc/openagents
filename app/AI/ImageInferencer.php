<?php

namespace App\AI;

use GuzzleHttp\Client;

class ImageInferencer
{
    public static function multimodalInference(string $input, ?string $model, callable $streamFunction, ?Client $httpClient = null): array
    {
        $decodedInput = json_decode($input, true);

        if (json_last_error() === JSON_ERROR_NONE && isset($decodedInput['images'])) {
            $text = $decodedInput['text'] ?? '';
            $images = $decodedInput['images'] ?: [];

            $inputForModel = [
                'text' => $text,
                'image_url' => $images[0] ?? null, // Taking the first image for simplicity
            ];

            $messages = self::prepareMultiModalInference($inputForModel);
        } else {
            $text = $input;
            $messages = self::prepareTextInference($text);
        }

        if (! $model) {
            //            $model = 'gpt-4o';
            $model = 'gpt-4-vision-preview';
        }

        $modelDetails = Models::MODELS[$model] ?? Models::MODELS['gpt-4-vision-preview'];

        if ($modelDetails) {
            $gateway = $modelDetails['gateway'];
            $maxTokens = $modelDetails['max_tokens'];

            $params = [
                'model' => $model,
                'messages' => $messages,
                'max_tokens' => $maxTokens,
                'stream_function' => $streamFunction,
            ];

            switch ($gateway) {
                case 'openai':
                    $client = new OpenAIGateway();
                    break;
                default:
                    dd("Unknown gateway: $gateway");
            }

            $inference = $client->inference($params);
        } else {
            dd("Unknown model: $model");
        }

        return $inference;
    }

    private static function prepareMultiModalInference($input): array
    {
        $systemMessage = [
            'role' => 'system',
            'content' => 'You are a helpful assistant.',
        ];

        $userMessageContent = [];

        if (! empty($input['text'])) {
            $userMessageContent[] = [
                'type' => 'text',
                'text' => $input['text'],
            ];
        }

        if (! empty($input['image_url'])) {
            $base64prefixedPng = 'data:image/png;base64,'.$input['image_url'];
            $userMessageContent[] = [
                'type' => 'image_url',
                'image_url' => $base64prefixedPng,
            ];
        }

        $userMessage = [
            'role' => 'user',
            'content' => $userMessageContent,
        ];

        return [$systemMessage, $userMessage];
    }

    private static function prepareTextInference($text): array
    {
        return [
            [
                'role' => 'system',
                'content' => 'You are a helpful assistant.',
            ],
            ['role' => 'user', 'content' => $text],
        ];
    }
}
