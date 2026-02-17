<?php

namespace App\AI\Tools;

use Illuminate\Contracts\JsonSchema\JsonSchema;
use Laravel\Ai\Contracts\Tool;
use Laravel\Ai\Tools\Request;

class EchoTool implements Tool
{
    public function name(): string
    {
        return 'echo';
    }

    public function description(): string
    {
        return 'Echo back the provided text. Useful for testing tool calls end-to-end.';
    }

    public function handle(Request $request): string
    {
        $args = $request->toArray();
        $text = $args['text'] ?? $args['message'] ?? null;
        if (is_string($text) && $text !== '') {
            return $text;
        }
        foreach ($args as $value) {
            if (is_string($value) && $value !== '') {
                return $value;
            }
        }

        return '(no text provided)';
    }

    public function schema(JsonSchema $schema): array
    {
        return [
            'text' => $schema
                ->string()
                ->description('Text to echo back.')
                ->required(),
        ];
    }
}
