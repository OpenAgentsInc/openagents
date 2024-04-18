<?php

namespace App\AI;

class ReplicateAIGateway
{
    private $apiToken;

    public function __construct()
    {
        $this->apiToken = env('REPLICATE_API_KEY');
    }

    public function predict($prompt, $streamFunction, $messages)
    {
        $formattedMessages = $this->formatMessagesForLlama($messages, $prompt);

        $input = json_encode([
            'stream' => true,
            'input' => [
                'prompt' => $formattedMessages,
                'prompt_template' => '',
            ],
        ]);

        //        $input = json_encode([
        //            'stream' => true,
        //            'input' => [
        //                'prompt' => $prompt,
        //                'prompt_template' => "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\nYou are a helpful assistant<|eot_id|><|start_header_id|>user<|end_header_id|>\n\n{prompt}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
        //            ],
        //        ]);

        $ch = curl_init('https://api.replicate.com/v1/models/meta/meta-llama-3-70b-instruct/predictions');
        curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
        curl_setopt($ch, CURLOPT_POST, true);
        curl_setopt($ch, CURLOPT_HTTPHEADER, [
            "Authorization: Bearer {$this->apiToken}",
            'Content-Type: application/json',
        ]);
        curl_setopt($ch, CURLOPT_POSTFIELDS, $input);

        $prediction = curl_exec($ch);
        curl_close($ch);

        if (! $prediction) {
            throw new Exception('Failed to get prediction from Replicate API.');
        }

        $predictionData = json_decode($prediction, true);
        $streamUrl = $predictionData['urls']['stream'];

        $content = '';
        $inputTokens = 0;
        $outputTokens = 0;

        $streamHandle = curl_init($streamUrl);
        curl_setopt($streamHandle, CURLOPT_HTTPHEADER, [
            'Accept: text/event-stream',
            'Cache-Control: no-store',
        ]);
        curl_setopt($streamHandle, CURLOPT_WRITEFUNCTION, function ($curl, $data) use ($streamFunction, &$content, &$inputTokens, &$outputTokens) {
            $lines = explode("\n", $data);
            foreach ($lines as $line) {
                if (strpos($line, 'data: ') === 0) {
                    $token = substr($line, 6);
                    $content .= $token;
                    $response = [
                        'choices' => [
                            [
                                'delta' => [
                                    'content' => $token,
                                ],
                            ],
                        ],
                    ];
                    $streamFunction($response);
                } elseif (strpos($line, 'input_tokens: ') === 0) {
                    $inputTokens = (int) substr($line, 14);
                } elseif (strpos($line, 'output_tokens: ') === 0) {
                    $outputTokens = (int) substr($line, 15);
                }
            }

            return strlen($data);
        });
        curl_exec($streamHandle);
        curl_close($streamHandle);

        return [
            'content' => $content,
            'input_tokens' => $inputTokens,
            'output_tokens' => $outputTokens,
        ];
    }

    private function formatMessagesForLlama(array $messages, string $prompt): string
    {
        $formattedMessages = '';

        foreach ($messages as $message) {
            $role = $message['role'];
            $content = $message['content'];

            if ($role === 'system') {
                $formattedMessages .= "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\n{$content}<|eot_id|>";
            } elseif ($role === 'user') {
                $formattedMessages .= "<|start_header_id|>user<|end_header_id|>\n\n{$content}<|eot_id|>";
            } elseif ($role === 'assistant') {
                $formattedMessages .= "<|start_header_id|>assistant<|end_header_id|>\n\n{$content}<|eot_id|>";
            }
        }

        $formattedMessages .= "<|start_header_id|>user<|end_header_id|>\n\n{$prompt}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n";

        return $formattedMessages;
    }
}
