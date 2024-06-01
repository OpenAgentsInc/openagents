<?php

namespace App\Utils;

use App\Grpc\nostr\PoolConnectorClient;
use App\Grpc\nostr\RpcDiscoverNearbyActionsRequest;
use App\Grpc\nostr\RpcDiscoverNearbyActionsResponse;
use App\Models\PoolJob;
use App\Services\OpenObserveLogger;
use App\Services\PoolService;
use Exception;
use Grpc\ChannelCredentials;
use Illuminate\Support\Facades\Log;
use Symfony\Component\Uid\Uuid;

class PoolUtils
{
    public static function uuid()
    {
        return Uuid::v4()->toRfc4122();
    }

    public static function sendRAGWarmUp($agentId, $threadId, $userId, $documents)
    {

        PoolUtils::sendRAGJob($agentId, $threadId, $userId, $documents, '', false, true);
    }

    public static function sendRAGJob($agentId, $threadId, $userId, $documents, $query, $withTools = false, $warmUp = false)
    {
        $logger = new OpenObserveLogger([

        ]);

        $job_id = (new PoolService())
            ->poolAddress(config('pool.address'))
            ->query($query)
            ->useTools($withTools)
            ->documents($documents)
            ->uuid('openagents.com-'.$userId.'-'.$threadId)
            ->warmUp($warmUp)
            ->cacheDurationhint(-1)
            ->encryptFor(config('pool.encrypt'))
            ->execute();

        $logger->log('info', 'Requesting '.($warmUp ? 'warm up' : '').'Job with ID: '.$job_id.' for Agent: '.$agentId.' Thread: '.$threadId);
        $job = new PoolJob();
        $job->agent_id = $agentId;
        $job->job_id = $job_id;
        $job->status = 'pending';
        $job->thread_id = $threadId;
        $job->warmup = $warmUp;
        $job->save();
        $logger->close();
    }

    public static function getTools()
    {
        $hostname = config('pool.address');
        $opts = [
            'credentials' => config('pool.address_ssl') ? ChannelCredentials::createSsl() : ChannelCredentials::createInsecure(),
            'update_metadata' => function ($metaData) {
                $metaData['authorization'] = [config('pool.node_token')];

                return $metaData;
            },
        ];

        $client = new PoolConnectorClient($hostname, $opts);
        try {
            $req = new RpcDiscoverNearbyActionsRequest();
            $req->setFilterByTags(['tool']);
            $req->setFilterByKindRanges(['5000-5999']);
            $req = $client->discoverNearbyActions($req);
            /** @var array [@var RpcDiscoverNearbyActionsResponse, status] */
            $res = $req->wait();
            $status = $res[1]->code;
            if ($status !== 0) {
                throw new Exception($res[1]->details);
            }

            /** @var RpcDiscoverNearbyActionsResponse */
            $discovered = $res[0];

            $tools = [];
            foreach ($discovered->getActions() as $actionStr) {
                $action = json_decode($actionStr, true);

                /** @var array<string, mixed> */
                $meta = $action['meta'];
                /** @var string */
                $template = $action['template'];
                /** @var array<string, mixed> */
                $sockets = $action['sockets'];
                /** @var string */
                $id = $meta['id'];
                $tools[] = [
                    'id' => $id,
                    'template' => $template,
                    'meta' => $meta,
                    'sockets' => $sockets,
                ];
            }

            return $tools;
        } catch (Exception $e) {
            Log::error('Error in requestJob: '.$e->getMessage());
            throw $e;
        } finally {
            $client->close();
        }
    }
}
