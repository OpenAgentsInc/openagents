<?php

namespace App\AI\Tools;

use Laravel\Ai\Contracts\Tool;

class ToolRegistry
{
    /**
     * @return Tool[]
     */
    public function all(): array
    {
        return [
            new GetTimeTool,
            new EchoTool,
        ];
    }
}
