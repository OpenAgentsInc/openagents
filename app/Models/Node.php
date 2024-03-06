<?php

namespace App\Models;

use App\Services\Inferencer;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Node extends Model
{
    use HasFactory;

    protected $guarded = [];

    // Each node can have multiple ports.
    public function ports(): HasMany
    {
        return $this->hasMany(Port::class);
    }

    /**
     * Triggers this node's logic.
     *
     * @param  array  $params  Parameters including 'input', 'streamingFunction', 'agent', 'flow', and 'thread'.
     * @return string The output string from the node.
     */
    public function trigger(array $params): string
    {
        // Extract parameters
        $input = $params['input'];
        $streamingFunction = $params['streamingFunction'];
        $agent = $params['agent'];
        $thread = $params['thread'];

        // Node-specific logic
        switch ($this->type) {
            case 'inference':
                // Call the Inferencer for LLM inference
                $output = Inferencer::llmInference($input, $thread, $agent, $streamingFunction);
                break;

            default:
                // Default processing logic for nodes
                $output = 'Default node processing for: '.$input;
                break;
        }

        return $output;
    }
}
