<?php

namespace App\Services;

use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Log;

class FlushLogEntriesJob implements ShouldQueue
{
    private $logEntries;

    private $options;

    public function __construct($logEntries, $options)
    {
        $this->logEntries = $logEntries;
        $this->options = $options;
    }

    public function handle()
    {
        $this->flushLoop($this->logEntries);
    }

    private function flushLoop($logEntries)
    {
        $username = env('OPENOBSERVE_USERNAME');
        $password = env('OPENOBSERVE_PASSWORD');
        $basicAuth = base64_encode("{$username}:{$password}");

        $url = $this->options['baseUrl'].'/api/'.$this->options['org'].'/'.$this->options['stream'].'/_json';

        $headers = [
            'Content-Type' => 'application/json',
            'Authorization' => "Basic {$basicAuth}",
        ];

        $response = Http::withHeaders($headers)->post($url, $logEntries);

        if ($response->status() != 200) {
            Log::error('Error flushing log: '.$response->status());
        }
    }
}
