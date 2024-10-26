<?php

namespace App\Services;

use GuzzleHttp\Client;
use Illuminate\Support\Facades\Log;

class GitHubService
{
    private $client;
    private $token;

    public function __construct()
    {
        $this->client = new Client([
            'base_uri' => 'https://api.github.com/',
            'headers' => [
                'Accept' => 'application/vnd.github.v3+json',
            ],
        ]);
        $this->token = env('GITHUB_TOKEN');
    }

    public function viewFile(string $owner, string $repo, string $path, string $branch = 'main'): array
    {
        try {
            $response = $this->client->get("repos/{$owner}/{$repo}/contents/{$path}", [
                'headers' => [
                    'Authorization' => "token {$this->token}",
                ],
                'query' => [
                    'ref' => $branch,
                ],
            ]);

            $content = json_decode($response->getBody()->getContents(), true);

            if (!isset($content['content'])) {
                throw new \Exception("File content not found in GitHub response");
            }

            $decodedContent = base64_decode($content['content']);

            return [
                'success' => true,
                'content' => $decodedContent,
                'sha' => $content['sha'], // Add this line to include the file's SHA
                'summary' => "Viewed file {$path} in {$owner}/{$repo} on branch {$branch}",
                'details' => "File contents have been retrieved from GitHub",
            ];
        } catch (\Exception $e) {
            Log::error("GitHub API error: " . $e->getMessage());
            return [
                'success' => false,
                'error' => "Failed to retrieve file from GitHub",
                'details' => $e->getMessage(),
            ];
        }
    }

    public function viewFolder(string $owner, string $repo, string $path, string $branch = 'main'): array
    {
        try {
            $response = $this->client->get("repos/{$owner}/{$repo}/contents/{$path}", [
                'headers' => [
                    'Authorization' => "token {$this->token}",
                ],
                'query' => [
                    'ref' => $branch,
                ],
            ]);

            $contents = json_decode($response->getBody()->getContents(), true);

            if (!is_array($contents)) {
                throw new \Exception("Invalid response from GitHub API");
            }

            return [
                'success' => true,
                'content' => $contents,
                'summary' => "Viewed folder contents at {$path} in {$owner}/{$repo} on branch {$branch}",
                'details' => "Folder contents have been retrieved from GitHub",
            ];
        } catch (\Exception $e) {
            Log::error("GitHub API error: " . $e->getMessage());
            return [
                'success' => false,
                'error' => "Failed to retrieve folder contents from GitHub",
                'details' => $e->getMessage(),
            ];
        }
    }

    public function createFile(string $owner, string $repo, string $path, string $content, string $branch, string $commitMessage): array
    {
        try {
            $response = $this->client->put("repos/{$owner}/{$repo}/contents/{$path}", [
                'headers' => [
                    'Authorization' => "token {$this->token}",
                ],
                'json' => [
                    'message' => $commitMessage,
                    'content' => base64_encode($content),
                    'branch' => $branch,
                ],
            ]);

            $result = json_decode($response->getBody()->getContents(), true);

            if (!isset($result['commit']['sha'])) {
                throw new \Exception("Commit SHA not found in GitHub response");
            }

            return [
                'success' => true,
                'commit_sha' => $result['commit']['sha'],
                'summary' => "Created file {$path} in {$owner}/{$repo} on branch {$branch}",
                'details' => "File has been successfully created on GitHub",
            ];
        } catch (\Exception $e) {
            Log::error("GitHub API error: " . $e->getMessage());
            return [
                'success' => false,
                'error' => "Failed to create file on GitHub",
                'details' => $e->getMessage(),
            ];
        }
    }

    public function rewriteFile(string $owner, string $repo, string $path, string $content, string $branch, string $commitMessage): array
    {
        try {
            // First, get the current file to retrieve its SHA
            $currentFile = $this->viewFile($owner, $repo, $path, $branch);
            if (!$currentFile['success']) {
                throw new \Exception("Failed to retrieve current file: " . ($currentFile['error'] ?? 'Unknown error'));
            }

            $response = $this->client->put("repos/{$owner}/{$repo}/contents/{$path}", [
                'headers' => [
                    'Authorization' => "token {$this->token}",
                ],
                'json' => [
                    'message' => $commitMessage,
                    'content' => base64_encode($content),
                    'sha' => $currentFile['sha'], // Use the SHA from the viewFile response
                    'branch' => $branch,
                ],
            ]);

            $result = json_decode($response->getBody()->getContents(), true);

            if (!isset($result['commit']['sha'])) {
                throw new \Exception("Commit SHA not found in GitHub response");
            }

            return [
                'success' => true,
                'commit_sha' => $result['commit']['sha'],
                'summary' => "Updated file {$path} in {$owner}/{$repo} on branch {$branch}",
                'details' => "File has been successfully updated on GitHub",
            ];
        } catch (\Exception $e) {
            Log::error("GitHub API error: " . $e->getMessage());
            return [
                'success' => false,
                'error' => "Failed to update file on GitHub",
                'details' => $e->getMessage(),
            ];
        }
    }
}
