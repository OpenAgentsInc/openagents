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
use Illuminate\Support\Facades\Cache;

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
        $lock = Cache::lock('JobResultReceiverJob-'.$this->job_id, 60);
        if ($lock->get()) {
            $logger = new OpenObserveLogger([

            ]);

            try {
                $poolJob = PoolJob::where('job_id', $this->job_id)->first();
                if ($poolJob) {

                    // any warmup message will warm up the agent
                    $isWarmUp = $poolJob->warmup;
                    $agent = Agent::find($poolJob->agent_id);

                    if ($isWarmUp) {
                        if (! $agent) {
                            throw new Exception('Agent not found '.$poolJob->agent_id);
                        }
                        $logger->log('info', 'Agent WarmUp completed for agent '.$poolJob->agent_id);
                        $agent->is_rag_ready = true;
                        $agent->save();
                        AgentRagReady::dispatch($poolJob->agent_id);
                    }

                    if (! $poolJob->content && $poolJob->status == '') { // set content only once
                        $logger->log('info', 'Found Job: '.$this->job_id.' propagating content of length '.strlen($this->content));
                        $logger->log('info', 'Propagating content '.$this->content);

                        $contentData = json_decode($this->content, true);
                        $meta = $contentData['meta'];
                        if (! $meta) {
                            return;
                        }
                        $content = $contentData['content'] ?? '';

                        $usedToolIds = $meta['usedTools'] ?? [];
                        $poolJob->content = $content;
                        $poolJob->status = 'success';

                        // Track plugin usage
                        $availableTools = PoolUtils::getTools();
                        $usedTools = [];
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
                                    $usedTools[] = $tool;
                                }
                            }
                        }

                        // For each used tool extract payment info and pay the author
                        $totalCost = 0;
                        foreach ($usedTools as $usedTool) {
                            $meta = $usedTool['meta'];

                            $sats = PoolUtils::getToolPriceInSats($usedTool);
                            if (! isset($meta['payment'])) {
                                continue;
                            }

                            $lnAddress = $meta['payment'];
                            if (strpos($lnAddress, 'lightning:') === 0) {
                                $lnAddress = substr($lnAddress, 10);

                                // Add payment request
                                $poolJob->paymentRequests()->create([
                                    'amount' => $sats,
                                    'protocol' => 'lightning',
                                    'currency' => 'bitcoin',
                                    'target' => $lnAddress,
                                    'paid' => false,
                                ]);
                                $logger->log('info', 'Added payment request for '.$sats.' sats to '.$lnAddress);
                                $totalCost += $sats;
                            }
                        }

                        // Track total cost for average stats
                        $agent->trackToolsCost($totalCost);
                        //

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
            $lock->release();

        } else {
            $this->release(1);
        }
    }
}
