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
    dump("ABout to go do a thing");
    $response = Http::withHeaders([
      'Authorization' => 'Bearer ' . $this->token,
    ])
        ->timeout(90)
        ->post($this->api_url . '/chat/completions', $data);

    dump("RESPONSE:");
    dump($response);
    dump("RESPONSE JSON:");
    dump($response->json());

    return $response->json();
  }
}
