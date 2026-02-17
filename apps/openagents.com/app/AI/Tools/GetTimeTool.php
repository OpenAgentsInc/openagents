<?php

namespace App\AI\Tools;

use Illuminate\Contracts\JsonSchema\JsonSchema;
use Laravel\Ai\Contracts\Tool;
use Laravel\Ai\Tools\Request;

class GetTimeTool implements Tool
{
    public function name(): string
    {
        return 'get_time';
    }

    public function description(): string
    {
        return 'Get the current time in ISO-8601 (UTC).';
    }

    public function handle(Request $request): string
    {
        return now()->utc()->toIso8601String();
    }

    /**
     * Return a minimal schema so the API receives a valid parameters object.
     * (Empty array causes OpenAI/Vercel gateway to reject: "expected record, received undefined".)
     */
    public function schema(JsonSchema $schema): array
    {
        return [
            '_' => $schema->string()->description('Unused; ensures parameters is sent as an object.'),
        ];
    }
}
