<?php

namespace App\Services;

class Faerie {
    public $owner;
    public $repo;

    public function __construct($owner = "ArcadeLabsInc", $repo = "openagents") {
        $this->owner = $owner;
        $this->repo = $repo;
    }

    public function repoHasOpenPR() {
        $url = "https://api.github.com/repos/{$this->owner}/{$this->repo}/pulls?state=open";
        $response = $this->curl($url);
        return count($response) > 0;
    }

    private function curl ($url) {
        $startTime = microtime(true); // Start time

        $curl = curl_init();
        curl_setopt_array($curl, [
            CURLOPT_RETURNTRANSFER => 1,
            CURLOPT_URL => $url,
            CURLOPT_USERAGENT => 'ArcadeLabsInc',
            CURLOPT_HTTPHEADER => [
                'Accept: application/vnd.github.v3+json',
                'Authorization: token ' . env('GITHUB_TOKEN'),
            ],
        ]);
        $response = curl_exec($curl);
        curl_close($curl);

        $endTime = microtime(true); // End time
        $duration = ($endTime - $startTime) * 1000; // Duration in milliseconds

        // You can return the duration along with the response
        // Or you can log it, print it, or handle it as needed
        echo "Request duration: " . $duration . " ms\n";

        return json_decode($response, true);
    }
}
