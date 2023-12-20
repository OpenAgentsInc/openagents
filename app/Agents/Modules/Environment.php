<?php

namespace App\Agents\Modules;

use App\Traits\UsesCurl;
use App\Traits\UsesLogger;
use GitHub;

class Environment
{
    use UsesCurl, UsesLogger;

    private $owner;
    private $repo;

    public function __construct($fullRepo)
    {
        $this->initializeLogger();
        $this->validateRepo($fullRepo);
    }

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

    // Get repo info
    public function getRepo()
    {
        $info = GitHub::repo()->show($this->owner, $this->repo);
        $this->logger->recordStep('Get repo data', null, $info);
        return $info;
    }

    // Get file contents of folder
    public function getFolderContents($path = null)
    {
        $contents = GitHub::repo()->contents()->show($this->owner, $this->repo, $path);
        $this->logger->recordStep('Get folder contents', $path, $contents);
        return $contents;
    }

    public function fetchMostRecentIssue()
    {
        $response = $this->curl("https://api.github.com/repos/{$this->owner}/{$this->repo}/issues?state=open");
        $issue = $response["response"][0];
        $this->logger->log('Fetch most recent issue', [], $issue);
        $this->issue = $issue;
        return $this->issue;
    }

    private function validateRepo($fullRepo)
    {
        $fullRepo = explode("/", $fullRepo);
        if (count($fullRepo) != 2) {
            throw new \Exception("Invalid repo string");
        }
        [$this->owner, $this->repo] = $fullRepo;
    }
}
