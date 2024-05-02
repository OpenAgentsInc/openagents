<?php

namespace App\Services;

use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Support\Facades\Bus;
use Illuminate\Support\Facades\Http;
use Log;

class OpenObserveLogger
{
    private $options;

    private $batchSize;

    private $flushInterval;

    private $buffer;

    public function __construct($options)
    {
        $this->options = $options;
        $this->batchSize = $options['batchSize'] ?? 21;
        $this->flushInterval = $options['flushInterval'] ?? 5000;
        $this->buffer = collect();
    }

    public function log($level, $message, $timestamp = null)
    {
        $logEntry = [
            'level' => $level,
            '_timestamp' => $timestamp ?? now()->timestamp * 1000,
            'message' => $message,
        ];

        if (isset($this->options['meta'])) {
            foreach ($this->options['meta'] as $key => $value) {
                $logEntry[$key] = $value;
            }
        }

        $this->buffer->push($logEntry);

        if ($this->buffer->count() >= $this->batchSize) {
            Bus::dispatch(new FlushLogEntriesJob($this->buffer->all(), $this->options));
            $this->buffer = collect(); // Reset the buffer
        }
    }
}

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
