<?php

namespace App\Jobs;

use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldBeUnique;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Foundation\Bus\Dispatchable;
use Illuminate\Queue\InteractsWithQueue;
use Illuminate\Queue\SerializesModels;

class StartFaerieRun implements ShouldQueue
{
    use Dispatchable, InteractsWithQueue, Queueable, SerializesModels;

    public $faerie;

    /**
     * Create a new job instance.
     */
    public function __construct($faerie)
    {
        $this->faerie = $faerie;
    }

    /**
     * Execute the job.
     */
    public function handle(): void
    {
        dump("HI FAERIE JOB REPORTING");
        dump($this->faerie);
    }
}
