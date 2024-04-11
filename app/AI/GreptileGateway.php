<?php

namespace App\AI;

use Illuminate\Support\Facades\Http;

class GreptileGateway
{
    private $greptileApiKey;

    private $githubToken;

    private $greptileBaseUrl = 'https://api.greptile.com/v2';

    public function __construct()
    {
        $this->greptileApiKey = config('services.greptile.api_key');
        $this->githubToken = config('services.github.token');
    }

    public function createRepository($repository = 'OpenAgentsInc/openagents')
    {
        $response = Http::withHeaders([
            'Authorization' => 'Bearer '.$this->greptileApiKey,
            'X-Github-Token' => $this->githubToken,
            'Content-Type' => 'application/json',
        ])->post($this->greptileBaseUrl.'/repositories', [
            'remote' => 'github',
            'repository' => $repository,
        ]);

        return $response->json();
    }
}
