<?php

namespace App\Jobs;

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
    public Brain $brain;

    /**
     * Create a new job instance.
     */
    public function __construct($text, $brain)
    {
        $this->text = $text;
        $this->brain = $brain;
    }

    /**
     * Execute the job.
     */
    public function handle(): void
    {
        $gateway = new QueenbeeGateway();
        $result = $gateway->createEmbedding($this->text);
        $embedding = $result[0]['embedding'];

        // Create a new Datapoint
        $datapoint = Datapoint::create([
            'brain_id' => $this->brain->id,
            'data' => $this->text,
            'embedding' => $embedding,
        ]);
    }
}
