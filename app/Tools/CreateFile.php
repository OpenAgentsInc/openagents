<?php

namespace App\Tools;

use App\Services\GitHubService;
use Illuminate\Support\Facades\Log;

class CreateFile
{
    public static function getDefinition(): array
    {
        return [
            'name' => 'create_file',
            'description' => 'Creates a new file at the given path with the provided content',
            'parameters' => [
                'type' => 'object',
                'properties' => [
                    'path' => [
                        'type' => 'string',
                        'description' => 'The path where the new file should be created',
                    ],
                    'content' => [
                        'type' => 'string',
                        'description' => 'The content of the new file',
                    ],
                    'owner' => [
                        'type' => 'string',
                        'description' => 'The owner of the repository',
                    ],
                    'repo' => [
                        'type' => 'string',
                        'description' => 'The name of the repository',
                    ],
                    'branch' => [
                        'type' => 'string',
                        'description' => 'The branch to create the file on. If not provided, the default branch from the context will be used.',
                    ],
                ],
                'required' => ['path', 'content', 'owner', 'repo'],
            ],
        ];
    }

    public static function execute(array $params): array
    {
        $path = $params['path'];
        $content = $params['content'];
        $owner = $params['owner'];
        $repo = $params['repo'];
        $branch = $params['branch'] ?? 'main';

        $githubService = new GitHubService();

        // Generate commit message
        $commitMessage = self::generateCommitMessage($path);

        // Create the file
        $result = $githubService->createFile($owner, $repo, $path, $content, $branch, $commitMessage);

        if (!$result['success']) {
            Log::error("CreateFile: Failed to create file", $result);
            return [
                'success' => false,
                'summary' => 'Failed to create file',
                'details' => $result['details'] ?? 'Unknown error occurred while creating the file.',
            ];
        }

        return [
            'success' => true,
            'summary' => "Created {$path}",
            'details' => "File {$path} has been successfully created in {$owner}/{$repo} on branch {$branch}. Commit SHA: {$result['commit_sha']}",
            'commitMessage' => $commitMessage,
            'newContent' => $content,
        ];
    }

    private static function generateCommitMessage(string $path): string
    {
        $filename = basename($path);
        return "Create file {$filename}";
    }
}
