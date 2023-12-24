<?php

namespace App\Events;

use Illuminate\Broadcasting\Channel;
use Illuminate\Contracts\Broadcasting\ShouldBroadcast;
use Illuminate\Foundation\Events\Dispatchable;
use Illuminate\Queue\SerializesModels;

class ChatTokenReceived implements ShouldBroadcast
{
    use Dispatchable, SerializesModels;

    /**
     * The token data to be broadcasted.
     *
     * @var array
     */
    public $tokenData;

    /**
     * Create a new event instance.
     *
     * @param array $tokenData The token data to be broadcasted
     */
    public function __construct(array $tokenData)
    {
        $this->tokenData = $tokenData;
    }

    /**
     * Get the channels the event should broadcast on.
     *
     * @return array<int, \Illuminate\Broadcasting\Channel>
     */
    public function broadcastOn(): array
    {
        return [
            new Channel('Chat')
        ];
    }

    /**
     * Get the data to broadcast.
     *
     * @return array
     */
    public function broadcastWith(): array
    {
        return $this->tokenData;
    }
}
