<?php

namespace App\Services;

use Illuminate\Support\Facades\Bus;
use Illuminate\Support\Facades\Log;

class OpenObserveLogger
{
    private $options;

    private $batchSize;

    private $flushInterval;

    private $buffer;

    private $job_id;

    public function __construct($options)
    {
        $this->options = $options;
        if ( ! isset($this->options['stream'])) {
            $isProduction = env('APP_ENV') === 'production';
            $this->options['stream'] = $isProduction ? 'logs' : 'playground_logs';
        }

        $this->batchSize = $options['batchSize'] ?? 21;
        $this->flushInterval = $options['flushInterval'] ?? 5000;
        $this->buffer = collect();
        $this->job_id = $options['jobId'] ?? '';
    }

    public function log($level, $message, $timestamp = null)
    {

        if (! env('OPENOBSERVE_USERNAME') || ! env('OPENOBSERVE_PASSWORD')) {
            if ($level == 'info') {
                Log::info('LOGGER '.$message);
            } elseif ($level == 'error') {
                Log::error('LOGGER '.$message);
            } elseif ($level == 'warning') {
                Log::warning('LOGGER '.$message);
            } else {
                Log::debug('LOGGER '.$message);
            }

            return;
        }

        $appEnv = env('APP_ENV');
        $appName = env('APP_NAME');
        $appDebug = env('APP_DEBUG');

        $logEntry = [
            'level' => $level,
            '_timestamp' => $timestamp ?? round(microtime(true) * 1000),
            'log' => $message,
            'appName' => "OpenAgents $appName",
            'appVersion' => "$appEnv".($appDebug ? ' (debug)' : ''),
            'jobId' => $this->job_id,
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

    public function close() // flush on close
    {
        if ($this->buffer->count() > 0) {
            Bus::dispatch(new FlushLogEntriesJob($this->buffer->all(), $this->options));
            $this->buffer = collect(); // Reset the buffer
        }
    }
}
