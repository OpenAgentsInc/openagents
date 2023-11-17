<?php

namespace App\Services;

use Illuminate\Support\Facades\Http;

class QueenbeeGateway {

  private $api_url;
  private $client;
  private $token;

  function __construct()
  {
    $this->api_url = "https://queenbee.gputopia.ai/v1";
    $this->token = "ac4a9ce1c028c7a1e652d11f4d7e009e";
  }

  public function makeChatCompletion($data)
  {
    $response = Http::withHeaders([
      'Authorization' => 'Bearer ' . $this->token,
    ])->post($this->api_url . '/chat/completions', $data);

    return $response->json();
  }

  public function defaultModel()
  {
    return 'vicuna-v1-7b-q4f32_0';
  }

  public function createEmbedding($text, $model = 'fastembed:BAAI/bge-base-en-v1.5', $user = 'default-user', $gpu_filter = []) {
    $data = [
      'input' => $text,
      'model' => $model,
      'encoding_format' => 'float',
    ];

    $maxRetries = 10;
    $retryCount = 0;

    while ($retryCount < $maxRetries) {
      try {
        $response = Http::withHeaders([
          'Authorization' => 'Bearer ' . $this->token,
          'Content-Type' => 'application/json'
        ])->timeout(3) // Set timeout to 3 seconds
          ->post($this->api_url . '/embeddings', $data);

        if ($response->getStatusCode() == 200) {
          $body = json_decode($response->getBody()->getContents(), true);
          return $body['data'];
        } else {
          $retryCount++;
        }
      } catch (\Exception $e) {
        // Retry on exception (e.g., timeout)
        $retryCount++;
      }
    }

    // Handle case where all retries failed
    return []; // or throw an exception
  }
}
