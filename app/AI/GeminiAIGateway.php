<?php

namespace App\AI;

class GeminiAIGateway
{
    public function inference(string $text): array
    {
        return [
            'text' => $text,
        ];
    }
}
