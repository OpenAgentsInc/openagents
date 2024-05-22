<?php

namespace App\Utils;

use App\Models\NostrJob;
use App\Services\NostrService;
use App\Services\OpenObserveLogger;
use Symfony\Component\Uid\Uuid;

class PoolUtils
{
    public static function uuid()
    {
        return Uuid::v4()->toRfc4122();
    }

    public static function sendRAGWarmUp($agentId, $threadId, $userId, $documents, $withTools = true)
    {

        PoolUtils::sendRAGJob($agentId, $threadId, $userId, $documents, '', $withTools, true);
    }

    public static function sendRAGJob($agentId, $threadId, $userId, $documents, $query, $withTools = true,$warmUp = false)
    {
        $logger = new OpenObserveLogger([

        ]);

        $job_id = (new NostrService())
            ->poolAddress(config('nostr.pool'))
            ->query($query)
            ->useTools($withTools)
            ->documents($documents)
            ->uuid('openagents.com-'.$userId.'-'.$threadId)
            ->warmUp($warmUp)
            ->cacheDurationhint(-1)
            ->encryptFor(config('nostr.encrypt'))
            ->execute();

        $logger->log('info', 'Requesting '.($warmUp ? 'warm up' : '').'Job with ID: '.$job_id.' for Agent: '.$agentId.' Thread: '.$threadId);
        $job = new NostrJob();
        $job->agent_id = $agentId;
        $job->job_id = $job_id;
        $job->status = 'pending';
        $job->thread_id = $threadId;
        $job->warmup = $warmUp;
        $job->save();
        $logger->close();
    }
}
