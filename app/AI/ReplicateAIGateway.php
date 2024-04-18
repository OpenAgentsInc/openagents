<?php

class ReplicateAIGateway implements AIGatewayInterface
{
    private $apiToken;

    public function __construct()
    {
        $this->apiToken = env('REPLICATE_API_TOKEN');
    }

    public function predict($prompt, $streamFunction)
    {
        $input = json_encode([
            'stream' => true,
            'input' => [
                'prompt' => $prompt,
                'prompt_template' => "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\nYou are a helpful assistant<|eot_id|><|start_header_id|>user<|end_header_id|>\n\n{prompt}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
            ],
        ]);

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

        // Assuming the response needs to be streamed back
        $streamHandle = curl_init($streamUrl);
        curl_setopt($streamHandle, CURLOPT_HTTPHEADER, [
            'Accept: text/event-stream',
            'Cache-Control: no-store',
        ]);
        curl_setopt($streamHandle, CURLOPT_RETURNTRANSFER, true);
        $streamResponse = curl_exec($streamHandle);
        curl_close($streamHandle);

        return $streamResponse;
    }
}
