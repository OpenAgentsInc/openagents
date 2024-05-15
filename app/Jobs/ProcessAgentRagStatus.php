<?php

namespace App\Jobs;

use Exception;
use App\Models\Agent;
use App\Models\AgentJob;
use App\Events\AgentRagReady;
use Illuminate\Bus\Queueable;
use App\Services\OpenObserveLogger;
use Illuminate\Queue\SerializesModels;
use Illuminate\Queue\InteractsWithQueue;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;

class ProcessAgentRagStatus implements ShouldQueue
{
    use Dispatchable, InteractsWithQueue, Queueable, SerializesModels;


    public $tries = 5;
    public $backoff = 2; // seconds between retri


    protected $job_id;
    protected $logger;

    /**
     * Create a new job instance.
     */
    public function __construct($job_id)
    {
        $this->job_id = $job_id;
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
        $retryInterval = 2; // seconds
        $agentJob = null;

        // fetch the nostr job
        while ($retryCount < $maxRetries) {
            // fetch the nostr job
            $agentJob = AgentJob::where('job_id', $this->job_id)->first();
            if ($agentJob) {
                break;
            }
            $retryCount++;
            sleep($retryInterval);
        }

            if ($agentJob) {
                // $agentJob->is_rag_ready = true;
                // $agentJob->save();

                $this->logger->log('info', 'Found AgentJob: ' . $this->job_id);

                $agent = Agent::find($agentJob->agent_id);
                if ($agent && !$agent->is_rag_ready) {
                    $agent->is_rag_ready = true;
                    $agent->save();
                    AgentRagReady::dispatch($agentJob);
                }
            } else {
                $this->logger->log('info', 'AgentJob not found: ' . $this->job_id);
                $this->fail();
            }
        } catch (\Exception $exception) {
            // Log the error
            $this->logger->log('error','AgentJob not found: ' .  $exception->getMessage());

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
    // public function failed(Exception $exception)
    // {
    //     // Handle failure logic, like logging
    //     $this->logger->log('critical', 'AgentJob not found within retries: ' . $this->job_id);
    // }

}
