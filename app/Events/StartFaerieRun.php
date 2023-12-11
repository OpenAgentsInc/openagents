<?php

namespace App\Events;

use Illuminate\Broadcasting\Channel;
use Illuminate\Broadcasting\InteractsWithSockets;
use Illuminate\Broadcasting\PresenceChannel;
use Illuminate\Broadcasting\PrivateChannel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcast;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

class StartFaerieRun
{
    use Dispatchable, InteractsWithSockets, SerializesModels;

    private $faerie;

    /**
     * Create a new event instance.
     */
    public function __construct($faerie)
    {
        dump("Constructing the event");
        $this->faerie = $faerie;
    }

    // handle the event
    public function handle()
    {
        dump("Handling the event");
        Log::info("Handling the event");

        // $this->gateway->makeChatCompletion($data);
    }

    /**
     * Get the channels the event should broadcast on.
     *
     * @return array<int, \Illuminate\Broadcasting\Channel>
     */
    // public function broadcastOn(): array
    // {
    //     return [
    //         new PrivateChannel('channel-name'),
    //     ];
    // }
}
