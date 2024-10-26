<?php

namespace App\Tools;

use App\Services\GitHubService;
use Illuminate\Support\Facades\Log;

class ViewFolder
{
    public static function getDefinition(): array
    {
        return [
            'name' => 'view_folder',
            'description' => 'View file/folder hierarchy at path (one level deep)',
            'parameters' => [
                'type' => 'object',
                'properties' => [
                    'path' => [
                        'type' => 'string',
                        'description' => 'The path to view the folder contents',
                    ],
                    'branch' => [
                        'type' => 'string',
                        'description' => 'The branch to view the folder contents from',
                    ],
                    'owner' => [
                        'type' => 'string',
                        'description' => 'The owner of the repository',
                    ],
                    'repo' => [
                        'type' => 'string',
                        'description' => 'The name of the repository',
                    ],
                ],
                'required' => ['path', 'owner', 'repo'],
            ],
        ];
    }

    public static function execute(array $params): array
    {
        $path = $params['path'];
        $owner = $params['owner'];
        $repo = $params['repo'];
        $branch = $params['branch'] ?? 'main';

        $githubService = new GitHubService();
        $result = $githubService->viewFolder($owner, $repo, $path, $branch);

        if (!$result['success']) {
            Log::error("ViewFolder: Failed to retrieve folder contents", $result);
            return $result;
        }

        $files = [];
        $directories = [];

        foreach ($result['content'] as $item) {
            if ($item['type'] === 'file') {
                $files[] = $item['name'];
            } elseif ($item['type'] === 'dir') {
                $directories[] = $item['name'];
            }
        }

        return [
            'success' => true,
            'content' => [
                'files' => $files,
                'directories' => $directories,
            ],
            'summary' => "Viewed folder contents at {$path} in {$owner}/{$repo} on branch {$branch}",
            'details' => "File/folder hierarchy at {$path} in {$owner}/{$repo} on branch {$branch} has been retrieved from GitHub."
        ];
    }
}
