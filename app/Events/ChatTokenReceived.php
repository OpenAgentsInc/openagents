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
    public $messageId;
    public $tokenId;
    public $conversationId;

    /**
     * Create a new event instance.
     *
     * @param array $tokenData The token data to be broadcasted
     */
    public function __construct(string $tokenData, int $messageId, int $tokenId, int $conversationId)
    {
        $this->tokenData = $tokenData;
        $this->messageId = $messageId;
        $this->tokenId = $tokenId;
        $this->conversationId = $conversationId;
    }

    /**
     * Get the channels the event should broadcast on.
     *
     * @return array<int, \Illuminate\Broadcasting\Channel>
     */
    public function broadcastOn(): array
    {
        return [
            new Channel('Conversation.' . $this->conversationId)
        ];
    }

    /**
     * Get the data to broadcast.
     *
     * @return array
     */
    public function broadcastWith(): array
    {
        return [
            'token' => $this->tokenData,
            'tokenId' => $this->tokenId,
            'messageId' => $this->messageId,
        ];
    }
}
