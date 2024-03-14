<?php

namespace App\AI;

use GuzzleHttp\Client;

class StabilityAIGateway
{
    private $client;

    private $apiKey;

    public function __construct()
    {
        $this->client = new Client();
        $this->apiKey = env('STABILITY_API_KEY');
    }

    public function text_to_image(string $input): string
    {
        $url = 'https://api.stability.ai/v1/generation/stable-diffusion-xl-1024-v1-0/text-to-image';

        $headers = [
            'Accept' => 'application/json',
            'Authorization' => 'Bearer '.$this->apiKey,
        ];

        $body = [
            'steps' => 40,
            'width' => 1024,
            'height' => 1024,
            'seed' => 0,
            'cfg_scale' => 5,
            'samples' => 1,
            'text_prompts' => [
                ['text' => $input, 'weight' => 1],
                ['text' => 'blurry, bad', 'weight' => -1],
            ],
        ];

        try {
            $response = $this->client->post($url, [
                'headers' => $headers,
                'json' => $body,
            ]);

            $responseData = json_decode($response->getBody(), true);

            // Assuming $responseData contains the response object
            $base64Image = $responseData['artifacts'][0]['base64'];
            $imageData = base64_decode($base64Image);

            // Create a data URI for the image
            $imageMimeType = 'image/png'; // Adjust this based on the actual image format
            $dataUri = 'data:'.$imageMimeType.';base64,'.base64_encode($imageData);

            return $dataUri;

            // TODO: Save the image and return the path

        } catch (RequestException $e) {
            // Handle errors
            //            dd($e->getMessage());
            return "I'm learning to create images, check back soon!";
        }

    }
}
