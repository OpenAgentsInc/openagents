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
            new LightningL402FetchTool,
            new LightningL402ApproveTool,
            new LightningL402PaywallCreateTool,
            new LightningL402PaywallUpdateTool,
            new LightningL402PaywallDeleteTool,
        ];
    }
}
