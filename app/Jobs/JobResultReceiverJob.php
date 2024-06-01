<?php

namespace App\Jobs;

use App\Events\AgentRagReady;
use App\Events\PoolJobReady;
use App\Models\Agent;
use App\Models\PoolJob;
use App\Services\OpenObserveLogger;
use App\Utils\PoolUtils;
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

        ]);

        try {
            $poolJob = PoolJob::where('job_id', $this->job_id)->first();
            if ($poolJob) {

                // any warmup message will warm up the agent
                $isWarmUp = $poolJob->warmup;
                if ($isWarmUp) {
                    $agent = Agent::find($poolJob->agent_id);
                    if (! $agent) {
                        throw new Exception('Agent not found '.$poolJob->agent_id);
                    }
                    $logger->log('info', 'Agent WarmUp completed for agent '.$poolJob->agent_id);
                    $agent->is_rag_ready = true;
                    $agent->save();
                    AgentRagReady::dispatch($poolJob->agent_id);
                }

                if (! $poolJob->content) { // set content only once
                    $logger->log('info', 'Found Job: '.$this->job_id.' propagating content of length '.strlen($this->content));
                    $logger->log('info', 'Propagating content '.$this->content);

                    $contentData = json_decode($this->content, true);
                    $content = $contentData['content'];
                    $meta = $contentData['meta'];
                    $usedToolIds = $meta['usedTools'];

                    // Track plugin usage
                    $availableTools = PoolUtils::getTools();
                    if (count($usedToolIds) > 0) {
                        foreach ($usedToolIds as $toolId) {
                            $tool = null;
                            foreach ($availableTools as $availableTool) {
                                $logger->log('info', 'Checking tool '.json_encode($availableTool).$toolId);
                                if (isset($availableTool['id']) && $availableTool['id'] == $toolId) {
                                    $logger->log('info', 'Found tool '.$toolId);
                                    $tool = $availableTool;
                                    break;
                                }
                            }
                            if (isset($tool)) {
                                $logger->log('info', 'Used tool '.$tool['meta']['name']);
                            }
                        }
                    }
                    /////

                    $poolJob->content = $content;
                    $poolJob->save();
                    PoolJobReady::dispatch($poolJob);
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
                    dispatch($newJob)->delay(now()->addMilliseconds($this->backoff));
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
