<?php

namespace App\Http\Resources\Api;

use Illuminate\Http\Request;
use Illuminate\Http\Resources\Json\JsonResource;

/** @mixin \App\Models\Shout */
class ShoutResource extends JsonResource
{
    /**
     * @return array<string, mixed>
     */
    public function toArray(Request $request): array
    {
        return [
            'id' => (int) $this->id,
            'zone' => $this->zone,
            'body' => (string) $this->body,
            'visibility' => (string) $this->visibility,
            'author' => [
                'id' => (int) $this->author->id,
                'name' => (string) $this->author->name,
                'handle' => (string) $this->author->handle,
                'avatar' => $this->author->avatar,
            ],
            'createdAt' => $this->created_at?->toISOString(),
            'updatedAt' => $this->updated_at?->toISOString(),
        ];
    }
}
