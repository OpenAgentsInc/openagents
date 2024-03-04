<?php

namespace Database\Seeders;

use App\Models\Agent;
use App\Models\File;
use App\Models\Flow;
use App\Models\Node;
use App\Models\Thread;
use App\Models\User;
use Illuminate\Database\Seeder;

class SuperSeeder2 extends Seeder
{
    /**
     * Run the database seeds.
     */
    public function run(): void
    {
        // Make a user
        $user = User::factory()->create();

        // Make an agent owned by the user
        $agent = Agent::factory()->create(['user_id' => $user->id]);

        // Make a bunch of threads
        $threads = Thread::factory()->count(5)->create();

        // Join the user to those threads
        // Join the agent to those threads
        foreach ($threads as $thread) {
            $thread->users()->attach($user->id);
            $thread->agents()->attach($agent->id);
        }

        // Make a plugin node - URL extractor
        $urlExtractorNode = Node::factory()->create(['type' => 'plugin', 'name' => 'URL Extractor']);

        // Make a plugin node - URL scraper
        $urlScraperNode = Node::factory()->create(['type' => 'plugin', 'name' => 'URL Scraper']);

        // Make an LLM node
        $llmNode = Node::factory()->create(['type' => 'LLM', 'name' => 'Language Model Processor']);

        // Create Ports for each Node, defining input/output connections
        // Assuming we have a function `createPort` to simplify Port creation
        $extractorOutputPort = createPort($urlExtractorNode->id, 'output');
        $scraperInputPort = createPort($urlScraperNode->id, 'input');
        $scraperOutputPort = createPort($urlScraperNode->id, 'output');
        $llmInputPort = createPort($llmNode->id, 'input');

        // Establish connections between Ports to define the flow of data
        // For simplicity, assume we have a function `connectPorts` that connects two Ports
        connectPorts($extractorOutputPort, $scraperInputPort);
        connectPorts($scraperOutputPort, $llmInputPort);

        // Make a flow of those nodes
        $flow = Flow::factory()->create();
        $flow->nodes()->attach([$urlExtractorNode->id, $urlScraperNode->id, $llmNode->id]);

        // User sends a message to the thread
        $message = Message::factory()->create(['thread_id' => $threads->first()->id, 'user_id' => $user->id]);

        // User adds a file to the thread
        $file = File::factory()->create(['thread_id' => $threads->first()->id]);

        // User triggers a run of flow on the thread
        // Assuming `triggerFlowRun` initiates the Flow, passing initial data (message, file) to the first Node's input Port
        triggerFlowRun($flow->id, ['message' => $message, 'file' => $file]);

        // Run is updated with node results
        // Assuming each Node execution updates the Run with its output, managed through Ports

        // User adds user2 to conversation
        $user2 = User::factory()->create();
        $threads->first()->users()->attach($user2->id);

        // User2 adds messages
        $message2 = Message::factory()->create(['thread_id' => $threads->first()->id, 'user_id' => $user2->id]);

        // User1 triggers run
        // This time, the Run might include processing User2's message, showing the dynamic nature of Flows and Ports
        triggerFlowRun($flow->id, ['message' => $message2]);
    }
}

/**
 * Placeholder function for creating a Port
 */
function createPort($nodeId, $type)
{
    // Implementation details would go here, returning the created Port instance
}

/**
 * Placeholder function for connecting two Ports
 */
function connectPorts($outputPortId, $inputPortId)
{
    // Implementation details for connecting Ports would go here
}

/**
 * Placeholder function for triggering a Flow Run
 */
function triggerFlowRun($flowId, $initialData)
{
    // Implementation details for initiating a Run with initial data would go here
}
