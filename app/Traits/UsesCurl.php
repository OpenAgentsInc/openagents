<?php

namespace App\Traits;

trait UsesCurl
{
    public function curl($url)
    {
        $startTime = microtime(true); // Start time

        $curl = curl_init();
        curl_setopt_array($curl, [
            CURLOPT_RETURNTRANSFER => 1,
            CURLOPT_URL => $url,
            CURLOPT_USERAGENT => 'OpenAgentsInc',
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
        // Log::info("Request duration: " . $duration . " ms\n");

        return [
            "response" => json_decode($response, true),
            "duration" => round($duration)
        ];
    }
}
