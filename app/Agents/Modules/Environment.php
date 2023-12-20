<?php

namespace App\Agents\Modules;

use App\Traits\UsesCurl;
use App\Traits\UsesLogger;
use GitHub;

/**
 * Manages operations related to a GitHub repository environment.
 */
class Environment
{
    use UsesCurl, UsesLogger;

    private $owner;
    private $repo;

    /**
     * Constructor.
     * Initializes logger and validates the GitHub repository string.
     *
     * @param string $fullRepo The full repository string in 'owner/repo' format.
     * @throws \Exception If the repository string is invalid.
     */
    public function __construct($fullRepo)
    {
        $this->initializeLogger();
        $this->validateRepo($fullRepo);
    }

    /**
     * Gets a summary of the repository environment.
     * Includes most recent issue, repository info, and folder contents.
     *
     * @return array Summary of the repository environment.
     */
    public function getSummary()
    {
        $this->logger->log("Getting summary for $this->owner/$this->repo");

        $summaryArray = [
            "issue" => $this->fetchMostRecentIssue(),
            "repo" => $this->getRepo(),
            "folderContents" => $this->getFolderContents(),
        ];

        $this->logger->log("Summary for $this->owner/$this->repo: ");
        $this->logger->log($summaryArray);

        return $summaryArray;
    }

    /**
     * Retrieves repository information from GitHub.
     *
     * @return array Repository information.
     */
    public function getRepo()
    {
        $info = GitHub::repo()->show($this->owner, $this->repo);
        $this->logger->recordStep('Get repo data', null, $info);
        return $info;
    }

    /**
     * Fetches the contents of a folder in the repository.
     *
     * @param string|null $path The path to the folder in the repository.
     * @return array Contents of the specified folder.
     */
    public function getFolderContents($path = null)
    {
        $contents = GitHub::repo()->contents()->show($this->owner, $this->repo, $path);
        $this->logger->recordStep('Get folder contents', $path, $contents);
        return $contents;
    }

    /**
     * Fetches the most recent open issue from the repository.
     *
     * @return array Most recent open issue.
     */
    public function fetchMostRecentIssue()
    {
        $response = $this->curl("https://api.github.com/repos/{$this->owner}/{$this->repo}/issues?state=open");
        $issue = $response["response"][0];
        $this->logger->log('Fetch most recent issue', [], $issue);
        $this->issue = $issue;
        return $this->issue;
    }

    /**
     * Validates the provided full repository string.
     * Splits the string into owner and repository name.
     *
     * @param string $fullRepo Full repository string.
     * @throws \Exception If the string format is invalid.
     */
    private function validateRepo($fullRepo)
    {
        $fullRepo = explode("/", $fullRepo);
        if (count($fullRepo) != 2) {
            throw new \Exception("Invalid repo string");
        }
        [$this->owner, $this->repo] = $fullRepo;
    }
}
