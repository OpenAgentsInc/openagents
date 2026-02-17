<?php

namespace App\Http\Resources\Api;

use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

/** @mixin \App\Models\Whisper */
class WhisperResource extends JsonResource
{
    /**
     * @return array<string, mixed>
     */
    public function toArray(Request $request): array
    {
        return [
            'id' => (int) $this->id,
            'body' => (string) $this->body,
            'sender' => [
                'id' => (int) $this->sender->id,
                'name' => (string) $this->sender->name,
                'handle' => (string) $this->sender->handle,
                'avatar' => $this->sender->avatar,
            ],
            'recipient' => [
                'id' => (int) $this->recipient->id,
                'name' => (string) $this->recipient->name,
                'handle' => (string) $this->recipient->handle,
                'avatar' => $this->recipient->avatar,
            ],
            'readAt' => $this->read_at?->toISOString(),
            'createdAt' => $this->created_at?->toISOString(),
            'updatedAt' => $this->updated_at?->toISOString(),
        ];
    }
}
