<?php

namespace App\Models;

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
     * @param  string  $input  The input string to the node.
     * @param  callable  $streamingFunction  A callback function for streaming the output.
     * @return string The output string from the node.
     */
    public function trigger(string $input, callable $streamingFunction): string
    {
        // Node-specific logic goes here.
        // The implementation will vary depending on the node's functionality.

        // Example logic:
        // Check the type of the node or other attributes to determine what action to perform.
        switch ($this->type) {
            case 'inference':
                // If it's an inference node, you might perform some AI inference here.
                // Use $input as your input data for the inference.

                // This is a placeholder for where you'd integrate with your AI service, like an AI gateway.
                $output = 'Inferred output for: '.$input;
                break;

            default:
                // Handle other types of nodes or default behavior.
                $output = 'Default node processing for: '.$input;
                break;
        }

        // Optionally, use the streaming function to stream part of the output.
        // $streamingFunction("Streaming part of the output...");

        // Return the final output of the node.
        return $output;
    }
}
