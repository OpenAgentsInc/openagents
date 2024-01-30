<?php

namespace App\Services;

use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\Http;

class GitHub
{
    private $owner;

    private $repo;

    private $token;

    private $maxDepth = 1;

    public function __construct($url)
    {
        $this->parseGitHubUrl($url);
        $this->token = env('GITHUB_TOKEN');
    }

    private function parseGitHubUrl($url)
    {
        $path = parse_url($url, PHP_URL_PATH);
        $segments = explode('/', trim($path, '/'));

        if (count($segments) >= 2) {
            $this->owner = $segments[0];
            $this->repo = $segments[1];
        } else {
            throw new \InvalidArgumentException('Invalid GitHub URL provided.');
        }
    }

    public function getRepositoryHierarchyMarkdown()
    {
        dd(config('cache.default'));
        $cacheKey = '1_repo_hierarchy_'.$this->owner.'_'.$this->repo;

        return Cache::remember($cacheKey, now()->addMinutes(60), function () {
            // Fetch the contents at the repository's root and convert to Markdown
            return $this->fetchContentsRecursivelyMarkdown('/', 0);
        });
    }

    /**
     * Recursively fetch the contents of directories and format as Markdown.
     *
     * @param  string  $path
     * @param  int  $depth
     * @return string
     */
    private function fetchContentsRecursivelyMarkdown($path, $depth)
    {
        if ($depth > $this->maxDepth) {
            return ''; // Stop recursion if maximum depth is reached
        }

        $contents = $this->fetchFromGitHub($path);
        $markdown = '';
        $indent = str_repeat('  ', $depth); // Two spaces per depth level

        foreach ($contents as $content) {
            dump("Fetching: {$content['path']}");
            if ($content['type'] === 'dir') {
                $markdown .= "{$indent}- **{$content['name']}**\n";
                $markdown .= $this->fetchContentsRecursivelyMarkdown($content['path'], $depth + 1);
            } else {
                $markdown .= "{$indent}- {$content['name']}\n";
            }
        }

        return $markdown;
    }

    private function fetchFromGitHub($path)
    {
        $response = Http::withHeaders([
            'Accept' => 'application/vnd.github+json',
            'Authorization' => 'Bearer '.$this->token,
            'X-GitHub-Api-Version' => '2022-11-28',
        ])->get("https://api.github.com/repos/{$this->owner}/{$this->repo}/contents/{$path}");

        if ($response->successful()) {
            return $response->json();
        } else {
            throw new \Exception('GitHub API request failed: '.$response->body());
        }
    }

    public function getRepositoryHierarchy()
    {
        $contents = $this->fetchFromGitHub('/');

        return $this->fetchContentsRecursively($contents, 0);
    }

    private function fetchContentsRecursively($contents, $currentDepth, $path = '')
    {
        dump($path);
        $result = [];
        $maxDepth = 2;

        if ($currentDepth >= $maxDepth) {
            return [];
        }

        foreach ($contents as $content) {
            if ($content['type'] === 'dir') {
                $subContents = $this->fetchFromGitHub($content['path']);
                $result[] = [
                    'type' => 'dir',
                    'name' => $content['name'],
                    'path' => $content['path'],
                    'contents' => $this->fetchContentsRecursively($subContents, $currentDepth + 1, $content['path']),
                ];
            } else {
                $result[] = [
                    'type' => 'file',
                    'name' => $content['name'],
                    'path' => $content['path'],
                ];
            }
        }

        return $result;
    }
}
