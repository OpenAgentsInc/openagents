<?php

namespace App\Jobs;

use App\Events\EmbeddingCreated;
use App\Models\Agent;
use App\Models\Brain;
use App\Models\Datapoint;
use App\Services\QueenbeeGateway;
use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldBeUnique;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;
use Illuminate\Queue\InteractsWithQueue;
use Illuminate\Queue\SerializesModels;

class CreateDatapointEmbedding implements ShouldQueue
{
    use Dispatchable, InteractsWithQueue, Queueable, SerializesModels;

    public string $text;
    public Agent $agent;
    public Brain $brain;

    /**
     * Create a new job instance.
     */
    public function __construct($text, $agent, $brain)
    {
        $this->text = $text;
        $this->agent = $agent;
        $this->brain = $brain;
    }

    /**
     * Execute the job.
     */
    public function handle(): void
    {
        $embedding = $this->generateEmbedding();

        // Create a new Datapoint
        $datapoint = Datapoint::create([
            'brain_id' => $this->brain->id,
            'data' => $this->text,
            'embedding' => $embedding,
        ]);

        $this->notifyAgent();
    }

    private function generateEmbedding()
    {
        // If testing, don't actually call the Queenbee API
        if (app()->environment('testing')) {
            // Create a fake embedding of 768 dimension - array
            $embedding = array_fill(0, 768, 0.5);
        } else {
            $gateway = new QueenbeeGateway();
            $result = $gateway->createEmbedding($this->text);
            $embedding = $result[0]['embedding'];
        }

        return $embedding;
    }

    private function notifyAgent()
    {
        broadcast(new EmbeddingCreated($this->agent->id));
    }
}
