<?php

namespace App\Agents\Modules;

use App\Traits\UsesLogger;

class Environment
{
    use UsesLogger;

    private $owner;
    private $repo;

    public function __construct($fullRepo)
    {
        $this->initializeLogger();

        // Validate fullRepo & split into owner & repo
        $fullRepo = explode("/", $fullRepo);
        if (count($fullRepo) != 2) {
            throw new \Exception("Invalid repo string");
        }
        $this->owner = $fullRepo[0];
        $this->repo = $fullRepo[1];
    }

    public function get($url)
    {
        $this->logger->log("Getting $url");

        $curl = curl_init();
        curl_setopt_array($curl, [
            CURLOPT_URL => $url,
            CURLOPT_RETURNTRANSFER => true,
            CURLOPT_HTTPHEADER => [
                "Accept: application/vnd.github.v3+json",
                "User-Agent: GitHub Agent",
            ],
        ]);
        $response = curl_exec($curl);
        curl_close($curl);

        // $this->logger->log("Response: $response");

        return $response;
    }

    public function getSummary()
    {
        $this->logger->log("Getting summary for $this->owner/$this->repo");

        $url = "https://api.github.com/repos/$this->owner/$this->repo";
        $response = $this->get($url);
        $response = json_decode($response, true);

        $summary = [
            "name" => $response["name"],
            "description" => $response["description"],
            "stars" => $response["stargazers_count"],
            "forks" => $response["forks_count"],
            "issues" => $response["open_issues_count"],
            "watchers" => $response["subscribers_count"],
            "contributors" => $response["contributors_url"],
            "languages" => $response["languages_url"],
            // "topics" => $response["topics_url"],
            "license" => $response["license"]["name"],
            "created" => $response["created_at"],
            "updated" => $response["updated_at"],
            "pushed" => $response["pushed_at"],
        ];

        $this->logger->log("Summary for $this->owner/$this->repo: " . json_encode($summary));

        return $summary;
    }
}
