<?php

namespace App\AI;

class PerplexityAIGateway
{
    private $client;

    public function __construct()
    {
        $this->client = new PerplexityClient();
    }

    public function createStreamed(array $params): array
    {
        return $this->client->createStreamed($params);
    }
}
