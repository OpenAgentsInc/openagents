<?php

namespace App\Jobs;

use App\Events\AgentRagReady;
use App\Events\NostrJobReady;
use App\Models\Agent;
use App\Models\NostrJob;
use App\Services\OpenObserveLogger;
use Exception;
use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;
use Illuminate\Queue\InteractsWithQueue;
use Illuminate\Queue\SerializesModels;

class JobResultReceiverJob implements ShouldQueue
{
    use Dispatchable, InteractsWithQueue, Queueable, SerializesModels;

    public $tries = 3;

    public $backoff = 200; // milliseconds between retries

    protected $job_id;

    protected $content;

    protected $payload;

    protected $retry;

    /**
     * Create a new job instance.
     */
    public function __construct($job_id, $content, $payload, $retry = 0)
    {
        $this->job_id = $job_id;
        $this->content = $content;
        $this->payload = $payload;
        $this->retry = $retry;

    }

    /**
     * Execute the job.
     */
    public function handle(): void
    {
        $logger = new OpenObserveLogger([
            'baseUrl' => 'https://pool.openagents.com:5080',
            'org' => 'default',
        ]);

        try {
            $nostr_job = NostrJob::where('job_id', $this->job_id)->first();
            if ($nostr_job) {

                // any warmup message will warm up the agent
                $isWarmUp = $nostr_job->warmup;
                if ($isWarmUp) {
                    $agent = Agent::find($nostr_job->agent_id);
                    if (! $agent) {
                        throw new Exception('Agent not found '.$nostr_job->agent_id);
                    }
                    $logger->log('info', 'Agent WarmUp completed for agent '.$nostr_job->agent_id);
                    $agent->is_rag_ready = true;
                    $agent->save();
                    AgentRagReady::dispatch($nostr_job->agent_id);
                }

                if (! $nostr_job->content) { // set content only once
                    $logger->log('info', 'Found Job: '.$this->job_id.' propagating content of length '.strlen($this->content));
                    $logger->log('info', 'Propagating content '.$this->content);
                    $nostr_job->content = $this->content;
                    $nostr_job->save();
                    NostrJobReady::dispatch($nostr_job);
                } else {
                    $logger->log('fine', 'Job already processed: '.$this->job_id);
                }

            } else {
                $logger->log('fine', 'Job not found: '.$this->job_id);
                // $this->fail();
                // reschedule
                if ($this->retry < $this->tries) {
                    $logger->log('info', 'Rescheduling Job: '.$this->job_id.' retry '.($this->retry + 1));
                    $newJob = new JobResultReceiverJob($this->job_id, $this->content, $this->payload, $this->retry + 1);
                    dispatch($newJob)->delay(now()->addMillis($this->backoff));
                } else {
                    $logger->log('error', 'Failed to process Job: '.$this->job_id);
                }
            }
        } catch (Exception $exception) {
            $logger->log('error', $exception->getMessage());
        } finally {
            $logger->close();
        }
    }
}
