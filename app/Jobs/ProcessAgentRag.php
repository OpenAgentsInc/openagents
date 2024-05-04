<?php

namespace App\Jobs;

use App\Models\Agent;
use App\Models\AgentJob;
use App\Models\AgentFile;
use Illuminate\Bus\Queueable;
use App\Services\NostrService;
use Illuminate\Queue\SerializesModels;
use Illuminate\Queue\InteractsWithQueue;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;

class ProcessAgentRag implements ShouldQueue
{
    use Dispatchable, InteractsWithQueue, Queueable, SerializesModels;


    protected $agent;

    /**
     * Create a new job instance.
     */
    public function __construct(Agent $agent)
    {
        $this->agent = $agent;
    }

    /**
     * Execute the job.
     */
    public function handle(): void
    {
        $documents = AgentFile::where('agent_id', $this->agent->id)->pluck('url')->toArray();

        // send to nostra

        $pool = config('nostr.pool');
        $query = "";
        $encrypt =  config('nostr.encrypt');

        $job_id = (new NostrService())
            ->poolAddress($pool)
            ->query($query)
            ->documents($documents)
            ->k(1)
            ->maxTokens(512)
            ->overlap(256)
            ->quantize(false)
            ->warmUp(true)
            ->cacheDurationhint('-1')
            ->encryptFor($encrypt)
            ->execute();

            $existing  = AgentJob::where('agent_id',$this->agent->id)->first();


            $rag = $existing ? $existing : new AgentJob();
            $rag->job_id = $job_id;
            $rag->agent_id = $this->agent->id;
            $rag->is_rag_ready = false;
            $rag->save();
    }
}
