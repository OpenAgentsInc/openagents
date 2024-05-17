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
    public static function inference(string $model, NostrJob $job, callable $streamFunction, ?Client $httpClient = null): array
    {
        $thread = Thread::find($job->thread_id);
        $agent = Agent::find($job->agent_id);

        $prePrompt = 'You can use the following CONTEXT to help you answer the user\'s questions.';

        $systemPrompt = $agent->prompt."\n".$prePrompt."\n".$job->content;

        $logger = new OpenObserveLogger([
            'baseUrl' => 'https://pool.openagents.com:5080',
            'org' => 'default',
            'jobId' => $job->job_id,
        ]);
        $logger->log('info', 'Using Augmented prompt '.$systemPrompt);
        $logger->close();
        // When prompt is empty it gets picked up from the thread if needed
        $inference = new SimpleInferencer();

        return $inference->inference('', $model, $thread, $streamFunction, $httpClient, $systemPrompt);
    }
}
