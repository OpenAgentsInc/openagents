<?php

namespace App\Services;

use Illuminate\Support\Facades\Bus;

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
            'log' => $message,
            'appName' => "OpenAgents Laravel",
            "appVersion" => "1.0",
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
