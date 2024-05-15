<?php

namespace App\Jobs;

use App\Events\AgentRagReady;
use App\Events\NostrJobReady;
use App\Models\Agent;
use App\Models\AgentJob;
use App\Models\NostrJob;
use App\Services\OpenObserveLogger;
use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;
use Illuminate\Queue\InteractsWithQueue;
use Illuminate\Queue\SerializesModels;

class NostrRagReady implements ShouldQueue
{
    use Dispatchable, InteractsWithQueue, Queueable, SerializesModels;

    protected $job_id;

    protected $content;

    protected $payload;

    /**
     * Create a new job instance.
     */
    public function __construct($job_id, $content, $payload)
    {
        $this->job_id = $job_id;
        $this->content = $content;
        $this->payload = $payload;
    }

    /**
     * Execute the job.
     */
    public function handle(): void
    {
        $logger = new OpenObserveLogger([
            'baseUrl' => 'https://pool.openagents.com:5080',
            'org' => 'default',
            'stream' => 'logs',
            'batchSize' => 1,
            'flushInterval' => 1000,
        ]);

        // Retry logic to check for the NostrJob
        $retryCount = 0;
        $maxRetries = 5;
        $retryInterval = 2; // seconds
        $nostr_job = null;

        // fetch the nostr job
        while ($retryCount < $maxRetries) {
            $nostr_job = NostrJob::where('job_id', $this->job_id)->first();
            if ($nostr_job) {
                $logger->log('info', 'Job not found: '.$this->job_id.' retrying in '.$retryInterval.' seconds... '.$retryCount.'/'.$maxRetries);
                break;
            }
            $retryCount++;
            sleep($retryInterval);
        }

        if ($nostr_job) {

            $nostr_job->content = $this->content;
            $nostr_job->save();

            $logger->log('info', 'Found NostrJob: '.$this->job_id.' on thread '.$nostr_job->thread_id.' propagating content of length '.strlen($this->content));
            $logger->log('info', 'Propagating content '.$this->content);

            // Dispatch a job to the thread_id using websocket
            NostrJobReady::dispatch($nostr_job);
        } else {
            $logger->log('fine', 'NostrJob not found: '.$this->job_id);

            $this->processAgent($this->job_id);
        }
    }

    public function processAgent($job_id)
    {

        // Retry logic to check for the NostrJob
        $retryCount = 0;
        $maxRetries = 5;
        $retryInterval = 2; // seconds
        $agentJob = null;

        // fetch the nostr job
        while ($retryCount < $maxRetries) {
            $agentJob = AgentJob::where('job_id', $job_id)->first();
            if ($agentJob) {
                break;
            }
            $retryCount++;
            sleep($retryInterval);
        }

        if ($agentJob) {
            // $agentJob->is_rag_ready = true;
            // $agentJob->save();

            $agent = Agent::find($agentJob->agent_id);
            if ($agent && ! $agent->is_rag_ready) {
                $agent->is_rag_ready = true;
                $agent->save();
                AgentRagReady::dispatch($agentJob);
            }
        }
    }
}
