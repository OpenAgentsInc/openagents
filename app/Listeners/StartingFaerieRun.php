<?php

namespace App\Listeners;

use App\Events\StartFaerieRun;
use Illuminate\Contracts\Queue\ShouldQueue;
use Illuminate\Queue\InteractsWithQueue;

class StartingFaerieRun
{
    /**
     * Create the event listener.
     */
    public function __construct()
    {
        //
    }

    /**
     * Handle the event.
     */
    public function handle(StartFaerieRun $event): void
    {
        dump("Handling the event!!!! faerie:");
        dump($event);

        $run = $event->faerie->run();
        dump("Run:");
        dump($run);
    }
}
