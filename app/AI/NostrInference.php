<?php

declare(strict_types=1);

namespace App\AI;

use App\Models\Agent;
use App\Models\NostrJob;
use App\Models\Thread;
use App\Services\OpenObserveLogger;
use GuzzleHttp\Client;

class NostrInference
{
    private ?Client $httpClient;

    public function __construct(?Client $httpClient = null)
    {
        $this->httpClient = $httpClient;
    }

    public function inference(string $model, NostrJob $job, callable $streamFunction): array
    {
        $thread = Thread::find($job->thread_id);
        $agent = Agent::find($job->agent_id);

        $prePrompt = 'You can use the following extracted parts of a long document to help you answer the user\'s questions.';

        $systemPrompt = $agent->prompt."\n".$prePrompt."\n".$job->content;

        $logger = new OpenObserveLogger([
            'jobId' => $job->job_id,
        ]);
        $logger->log('info', 'Using Augmented prompt '.$systemPrompt);
        $logger->close();

        $inferencer = new SimpleInferencer($this->httpClient);

        // When prompt is empty it gets picked up from the thread if needed
        return $inferencer->inference('', $model, $thread, $streamFunction, $systemPrompt);
    }
}
