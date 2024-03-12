<?php

namespace App\Models;

use App\Services\Inferencer;
use Exception;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Node extends Model
{
    use HasFactory;

    protected $fillable = ['name', 'description', 'type', 'config'];

    /**
     * Triggers this node's logic.
     *
     * @param  array  $params  Parameters including 'input', 'streamingFunction', 'agent', 'flow', and 'thread'.
     * @return string The output string from the node.
     *
     * @throws Exception
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
                $output = Inferencer::llmInference($agent, $this, $thread, $input, $streamingFunction);
                break;

            default:
                // Default processing logic for nodes
                $output = 'Default node processing for: '.$input;
                break;
        }

        return $output;
    }
}
