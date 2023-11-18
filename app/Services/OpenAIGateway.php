<?php

namespace App\Services;

use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Http;
use OpenAI;

class OpenAIGateway {

  private $client;
  private $token;

  function __construct()
  {
    $this->token = env('OPENAI_API_KEY');
    $this->client = OpenAI::client($this->token);
  }

}
