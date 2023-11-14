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

  public function createEmbedding($text, $model = 'fastembed:BAAI/bge-base-en-v1.5', $user = 'default-user', $gpu_filter = []) {
    $data = [
      'input' => $text,
      'model' => $model,
      'encoding_format' => 'float',
    ];

    $response = Http::withHeaders([
      'Authorization' => 'Bearer ' . $this->token,
      'Content-Type' => 'application/json'
    ])->post($this->api_url . '/embeddings', $data);

    // Parse the response
    if ($response->getStatusCode() == 200) {
      $body = json_decode($response->getBody()->getContents(), true);
      return $body['data'][0]['embedding'] ?? [];
    } else {
      // Handle error case
      return []; // or throw an exception
    }
  }
}
