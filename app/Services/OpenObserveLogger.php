<?php

namespace App\Services;

use Illuminate\Support\Facades\Log;

use Illuminate\Support\Facades\Bus;

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
        $this->batchSize = $options['batchSize'] ?? 21;
        $this->flushInterval = $options['flushInterval'] ?? 5000;
        $this->buffer = collect();
        $this->job_id = $options['jobId'] ?? '';
    }

    public function log($level, $message, $timestamp = null)
    {
        $logEntry = [
            'level' => $level,
            '_timestamp' => $timestamp ?? now()->timestamp * 1000,
            'log' => $message,
            'appName' => 'OpenAgents Laravel',
            'appVersion' => '1.0',
            'jobId' => $this->job_id,
        ];

        // if($level=="info"){
        //     Log::info("LOGGER ".$message);
        // }else if($level=="error"){
        //     Log::error("LOGGER " .$message);
        // }else if($level=="warning"){
        //     Log::warning("LOGGER " .$message);
        // }else{
        //     Log::debug("LOGGER " .$message);
        // }


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
