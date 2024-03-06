<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class Run extends Model
{
    use HasFactory;

    protected $guarded = [];

    // belongs to a flow
    public function flow(): BelongsTo
    {
        return $this->belongsTo(Flow::class);
    }

    /**
     * Triggers the execution of the associated flow.
     *
     * @param  callable  $streamingFunction  A callback function for streaming the output.
     * @return string The aggregated result of the flow execution.
     */
    public function trigger($streamingFunction)
    {
        // Ensure the flow is loaded along with its nodes
        $flow = $this->load('flow.nodes');

        $input = $this->input; // Assume this run model has an 'input' attribute.
        $output = '';

        // Execute each node in sequence (assuming nodes are already sorted by their execution order)
        foreach ($flow->nodes as $node) {
            // Call the node's trigger method and pass the current input
            $nodeOutput = $node->trigger($input);

            // Optional: Use streamingFunction to stream output of each node if necessary
            // $streamingFunction($nodeOutput);

            // Set the output of the current node as the input for the next node
            $input = $nodeOutput;

            // Append the output to the overall run output (this can be adjusted based on your needs)
            $output .= $nodeOutput;
        }

        // Return the final aggregated output
        return $output;
    }
}
