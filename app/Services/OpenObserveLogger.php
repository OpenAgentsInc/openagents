<?php

namespace App\Services;

use Exception;
use Illuminate\Queue\InteractsWithQueue;
use Illuminate\Queue\SerializesModels;
use Illuminate\Queue\Worker;
use Illuminate\Support\Collection;
use Illuminate\Support\Facades\Http;
use Log;

class OpenObserveLogger
{
    use InteractsWithQueue, SerializesModels;

    private $options;

    private $batchSize;

    private $flushInterval;

    private $buffer;

    private $flushThread;

    public function __construct($options)
    {
        $this->options = $options;
        $this->batchSize = $options['batchSize'] ?? 21;
        $this->flushInterval = $options['flushInterval'] ?? 5000;
        $this->buffer = new Collection();
        $this->flushThread = new Worker();
        $this->flushThread->daemon(function () {
            $this->flushLoop();
        });
    }

    private function flushLoop()
    {
        while (true) {
            sleep($this->flushInterval / 1000);

            $batch = $this->buffer->all();
            $this->buffer->clear();

            try {
                $url = $this->options['baseUrl'].'/api/'.$this->options['org'].'/'.$this->options['stream'].'/_json';

                $username = app()->env('OPENOBSERVE_USERNAME');
                $password = app()->env('OPENOBSERVE_PASSWORD');
                $basicAuth = base64_encode("{$username}:{$password}");

                $headers = [
                    'Content-Type' => 'application/json',
                    'Authorization' => "Basic {$basicAuth}",
                ];

                $response = Http::withHeaders($headers)->post($url, $batch);

                if ($response->status() != 200) {
                    Log::error('Error flushing log: '.$response->status());
                }
            } catch (Exception $e) {
                Log::error('Error flushing log: '.$e->getMessage());
            }
        }
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
            $this->flushThread->ping();
        }
    }
}
