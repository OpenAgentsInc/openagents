<?php

namespace App\Services;

use GrahamCampbell\GitHub\Facades\GitHub as GitHubClient;

class GitHub
{
    private $owner;

    private $repo;

    /**
     * Constructor that accepts a GitHub repository URL and initializes the class.
     *
     * @param  string  $url  The URL of the GitHub repository.
     */
    public function __construct($url)
    {
        $this->parseGitHubUrl($url);
        // $this->repo = GitHubClient::repo()->show($this->owner, $this->repo);
    }

    /**
     * Parse the GitHub URL to extract the owner and repository names.
     *
     * @param  string  $url  The GitHub URL.
     */
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

    /**
     * Get the file and folder hierarchy of the repository.
     *
     * @return array
     */
    public function getRepositoryHierarchy()
    {
        // Fetch the contents at the repository's root
        $contents = GitHubClient::repo()->contents()->show($this->owner, $this->repo, '/');
        dd($contents);

        // Recursively fetch the contents of each directory
        return $this->fetchContentsRecursively($contents, $this->owner, $this->repo);
    }

    /**
     * Recursively fetch the contents of directories.
     *
     * @param  array  $contents
     * @param  string  $owner
     * @param  string  $repo
     * @param  string  $path
     * @return array
     */
    private function fetchContentsRecursively($contents, $owner, $repo, $path = '')
    {
        $result = [];

        foreach ($contents as $content) {
            if ($content['type'] === 'dir') {
                // Fetch contents of the directory
                $subContents = GitHubClient::repo()->contents()->show($owner, $repo, $content['path']);
                $result[] = [
                    'type' => 'dir',
                    'name' => $content['name'],
                    'path' => $content['path'],
                    'contents' => $this->fetchContentsRecursively($subContents, $owner, $repo, $content['path']),
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
