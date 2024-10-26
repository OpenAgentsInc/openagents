<?php

namespace App\Tools;

use App\Services\GitHubService;

class ViewFile
{
    public static function getDefinition(): array
    {
        return [
            'type' => 'function',
            'function' => [
                'name' => 'view_file',
                'description' => 'View file contents at path',
                'parameters' => [
                    'type' => 'object',
                    'properties' => [
                        'path' => [
                            'type' => 'string',
                            'description' => 'The full path of the file to view',
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
                            'description' => 'The branch to view the file from',
                        ],
                    ],
                    'required' => ['path', 'owner', 'repo'],
                ],
            ],
        ];
    }

    public static function getBedrockDefinition(): array
    {
        return [
            'name' => 'view_file',
            'description' => 'View file contents at path',
            'parameters' => [
                'type' => 'object',
                'properties' => [
                    'path' => [
                        'type' => 'string',
                        'description' => 'The full path of the file to view',
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
                        'description' => 'The branch to view the file from',
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
        $result = $githubService->viewFile($owner, $repo, $path, $branch);

        return $result;
    }
}
