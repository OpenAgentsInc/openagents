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

    public function getRepository($repositoryId = 'github:main:OpenAgentsInc/openagents')
    {
        $encodedRepositoryId = rawurlencode($repositoryId);

        $response = Http::withHeaders([
            'Authorization' => 'Bearer '.$this->greptileApiKey,
            'Accept' => 'application/json',
        ])->get($this->greptileBaseUrl.'/repositories/'.$encodedRepositoryId);

        if ($response->successful() && $response->body()) {
            return $response->json();
        } else {
            // Handle error or empty response
            dd($response->body());
        }
    }
}
