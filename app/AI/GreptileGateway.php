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

    public function queryRepository($sessionId = '123345')
    {
        $messages = [
            'content' => 'Summarize this repo.',
            'role' => 'user',
        ];

        $data = [
            'messages' => $messages,
            'repositories' => [
                [
                    'remote' => 'github',
                    'branch' => 'main',
                    'repository' => 'OpenAgentsInc/openagents',
                ],
            ],
            'sessionId' => $sessionId,
            //            'stream' => true,
        ];

        $response = Http::withHeaders([
            'Authorization' => 'Bearer '.$this->greptileApiKey,
            'Content-Type' => 'application/json',
            'X-GitHub-Token' => $this->githubToken,
        ])->post($this->greptileBaseUrl.'/query', $data);

        if ($response->successful() && $response->body()) {
            return $response->json();
        } else {
            // Handle error or empty response
            dd($response->body());
        }
    }
}
