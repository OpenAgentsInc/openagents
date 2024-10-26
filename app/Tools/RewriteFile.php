<?php

namespace App\Tools;

use App\Services\GitHubService;
use Illuminate\Support\Facades\Log;

class RewriteFile
{
    public static function getDefinition(): array
    {
        return [
            'name' => 'rewrite_file',
            'description' => 'Rewrites the contents of a file at the given path',
            'parameters' => [
                'type' => 'object',
                'properties' => [
                    'path' => [
                        'type' => 'string',
                        'description' => 'The path of the file to rewrite',
                    ],
                    'content' => [
                        'type' => 'string',
                        'description' => 'The new content to write to the file',
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
                        'description' => 'The branch to update',
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

        // First, get the current file content
        $currentFileResult = $githubService->viewFile($owner, $repo, $path, $branch);

        if (!$currentFileResult['success']) {
            Log::error("RewriteFile: Failed to retrieve current file content", $currentFileResult);
            return [
                'success' => false,
                'summary' => 'Failed to rewrite file due to error retrieving current content',
                'details' => $currentFileResult['details'] ?? 'Unknown error occurred while retrieving file content.',
            ];
        }

        $oldContent = $currentFileResult['content'];

        // Generate commit message
        $commitMessage = self::generateCommitMessage($path, $oldContent, $content);

        // Rewrite the file
        $result = $githubService->rewriteFile($owner, $repo, $path, $content, $branch, $commitMessage);

        if (!$result['success']) {
            Log::error("RewriteFile: Failed to rewrite file", $result);
            return $result;
        }

        return [
            'success' => true,
            'summary' => "Edited {$path} - {$commitMessage}",
            'details' => "File {$path} has been successfully updated in {$owner}/{$repo} on branch {$branch}. Commit SHA: {$result['commit_sha']}",
            'commitMessage' => $commitMessage,
            'newContent' => $content,
            'oldContent' => $oldContent,
        ];
    }

    private static function generateCommitMessage(string $path, string $oldContent, string $newContent): string
    {
        $filename = basename($path);
        $oldLines = explode("\n", $oldContent);
        $newLines = explode("\n", $newContent);
        $addedLines = count($newLines) - count($oldLines);

        if ($addedLines > 0) {
            return "Update {$filename}: Add {$addedLines} lines";
        } elseif ($addedLines < 0) {
            return "Update {$filename}: Remove " . abs($addedLines) . " lines";
        } else {
            return "Update {$filename}: Modify content";
        }
    }
}
