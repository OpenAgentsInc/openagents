<?php

namespace App\Services;

use Illuminate\Support\Facades\Http;

class GitHub
{
    protected $baseUrl = 'https://api.github.com';

    public function __construct()
    {
        $this->token = env('GITHUB_TOKEN');
    }

    public function getRepositories(string $username): array
    {
        $response = Http::get("{$this->baseUrl}/users/{$username}/repos");

        return $response->successful() ? $response->json() : [];
    }

    public function getFileContents(string $owner, string $repo, string $path): array
    {
        $response = Http::withHeaders([
            'Accept' => 'application/vnd.github+json',
            'Authorization' => "Bearer {$this->token}",
            'X-GitHub-Api-Version' => '2022-11-28',
        ])->get("{$this->baseUrl}/repos/{$owner}/{$repo}/contents/{$path}");

        return $response->successful() ? $response->json() : ['error' => 'Failed to fetch file contents'];
    }
}
