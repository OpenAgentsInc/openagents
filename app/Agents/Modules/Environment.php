<?php

namespace App\Agents\Modules;

use App\Traits\UsesCurl;
use App\Traits\UsesLogger;

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
        $summary = $this->fetchMostRecentIssue();
        $this->logger->log("Summary for $this->owner/$this->repo: " . json_encode($summary));
        return $summary;
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
