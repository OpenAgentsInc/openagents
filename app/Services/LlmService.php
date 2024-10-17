<?php

namespace App\Services;

class LlmService
{
    public function processMessage(string $message): array
    {
        // Mock implementation
        return [
            'action' => 'create_file',
            'params' => [
                'path' => 'test.txt',
                'content' => 'Test content',
            ],
        ];
    }
}