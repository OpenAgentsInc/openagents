<?php

namespace App\Jobs;

use App\Events\NostrJobReady;
use App\Models\NostrJob;
use App\Services\OpenObserveLogger;
use Exception;
use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;
use Illuminate\Queue\InteractsWithQueue;
use Illuminate\Queue\SerializesModels;

class ProcessNostrRagReady implements ShouldQueue
{
    use Dispatchable, InteractsWithQueue, Queueable, SerializesModels;

    public $tries = 5;

    public $backoff = 2; // seconds between retries

    protected $job_id;

    protected $content;

    protected $payload;

    protected $logger;

    /**
     * Create a new job instance.
     */
    public function __construct($job_id, $content, $payload)
    {
        $this->job_id = $job_id;
        $this->content = $content;
        $this->payload = $payload;
        $this->logger = new OpenObserveLogger([
            'baseUrl' => 'https://pool.openagents.com:5080',
            'org' => 'default',
            'stream' => 'logs',
            'batchSize' => 1,
            'flushInterval' => 1000,
        ]);
    }

    /**
     * Execute the job.
     */
    public function handle(): void
    {

        try {
            // Retry logic to check for the NostrJob
            $retryCount = 0;
            $maxRetries = 5;
            $retryInterval = 3; // seconds
            $nostr_job = null;

            // fetch the nostr job
            while ($retryCount < $maxRetries) {
                $nostr_job = NostrJob::where('job_id', $this->job_id)->first();
                if ($nostr_job) {
                    break;
                }
                $retryCount++;
                sleep($retryInterval);
            }

            if ($nostr_job) {
                if (! $nostr_job->content) { // only once

                    $this->logger->log('info', 'Found NostrJob: '.$this->job_id.' propagating content of length '.strlen($this->content));
                    $this->logger->log('info', 'Propagating content '.$this->content);

                    // update the model payload and content
                    // $nostr_job->payload = $payload;
                    $nostr_job->content = $this->content;
                    $nostr_job->save();

                    // Dispatch a job to the thread_id using websocket
                    NostrJobReady::dispatch($nostr_job);
                } else {
                    $this->logger->log('fine', 'NostrJob already processed: '.$this->job_id);
                }

            } else {
                $this->logger->log('info', 'NostrJob not found: '.$this->job_id);
                $this->fail();
            }
        } catch (Exception $exception) {
            // Log the error
            $this->logger->log('error', $exception->getMessage());

            // Optionally, re-dispatch the job to retry immediately
            // Note: This will bypass the default retry mechanism.
            // dispatch(new self($this->job_id, $this->content, $this->payload))->delay(now()->addSeconds(2));

            // Rethrow the exception to let Laravel handle the retry
            throw $exception;
        }
    }

    /**
     * Handle a job failure.
     *
     * @return void
     */
    // public function failed(\Exception $exception)
    // {
    //     // Handle failure logic, like logging
    //     $this->logger->log('critical', 'NostrJob not found within retries: ' . $this->job_id);
    // }
}
