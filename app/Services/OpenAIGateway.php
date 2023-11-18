<?php

namespace App\Services;

use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Http;
use OpenAI;

class OpenAIGateway {

  private $api_url;
  private $client;
  private $token;

  function __construct()
  {
    $this->api_url = "https://api.openai.com/v1";
    $this->token = env('OPENAI_API_KEY');
    // $this->client = OpenAI::client($this->token);
  }

  public function defaultModel()
  {
    return 'gpt-3.5-turbo';
  }

  public function makeChatCompletion($data)
  {
    $response = Http::withHeaders([
      'Authorization' => 'Bearer ' . $this->token,
    ])->post($this->api_url . '/chat/completions', $data);

    return $response->json();
  }
}
