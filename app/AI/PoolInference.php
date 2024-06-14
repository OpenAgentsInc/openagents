<?php

declare(strict_types=1);

namespace App\AI;

use App\Models\Agent;
use App\Models\PoolJob;
use App\Models\Thread;
use App\Services\OpenObserveLogger;
use GuzzleHttp\Client;

class PoolInference
{
    private ?Client $httpClient;

    public function __construct(?Client $httpClient = null)
    {
        $this->httpClient = $httpClient;
    }

    public function inference(string $model, PoolJob $job, callable $streamFunction): array
    {
        $thread = Thread::find($job->thread_id);
        $agent = Agent::find($job->agent_id);

        $systemPrompt = implode("\n", [
            "Your name is {$agent->name}.",
            "Your description is: {$agent->about}",
            "Your instructions are: {$agent->prompt}.",
        ]);

        if ($job->content) {
            $systemPrompt .= "\nYou can use the following extracted parts of a long document to help you answer the user\'s questions:\n".$job->content;
        }

        $logger = new OpenObserveLogger([
            'jobId' => $job->job_id,
        ]);
        $logger->log('info', 'Using Agent prompt '.$systemPrompt);
        $logger->close();

        $inferencer = new SimpleInferencer($this->httpClient);

        // When prompt is empty it gets picked up from the thread if needed
        return $inferencer->inference('', $model, $thread, $streamFunction, $systemPrompt);
    }
}
