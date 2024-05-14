<?php

namespace App\Http\Controllers\Webhook;

use App\Events\AgentRagReady;
use App\Events\NostrJobReady;
use App\Http\Controllers\Controller;
use App\Models\Agent;
use App\Models\AgentJob;
use App\Models\NostrJob;
use App\Services\OpenObserveLogger;
use Illuminate\Http\Request;

class NostrHandlerController extends Controller
{
    public function handleEvent(Request $request)
    {
        $logger = new OpenObserveLogger([
            'baseUrl' => 'https://pool.openagents.com:5080',
            'org' => 'default',
            'stream' => 'logs',
            'batchSize' => 1,
            'flushInterval' => 1000,
        ]);

        $data = $request->all();

        $secret = $request->query('secret');
        $main_secret = config('nostr.webhook_secret');

        if ($secret !== $main_secret) {
            return response()->json(['error' => 'Invalid token'], 403);
        }

        $logData = $data;

        if ($logData[0] == 'Job') {
            $payload = $logData[1];
            $status = $payload['state']['status'];

            $extractedData = [
                'status' => $payload['state']['status'],
                'kind' => $payload['kind'],
            ];

            if (isset($payload['tags'])) {
                $extractedData['tags'] = [];
                foreach ($payload['tags'] as $tag) {
                    $extractedData['tags'][$tag[0]] = $tag[1];
                }
            }

            //            Log::channel('slack')->info(json_encode($extractedData));

            if ($status == 2) {
                $logger->log('info', 'Event with status 2: '.json_encode($payload));
                // log the error
                //                Log::error($data);
                //                Log::channel('slack')->error(json_encode($extractedData));

                return [
                    'status' => 'success',
                    'message' => 'error logged',
                ];
            } elseif ($status == 3) {

                $logger->log('info', 'Event with status 3: '.json_encode($payload));

                $job_id = $payload['id'];
                $content = $payload['result']['content'];
                $result = [
                    'payload' => $payload,
                    'job_id' => $job_id,
                    'content' => $content,
                ];

                // fetch the nostr job
                $nostr_job = NostrJob::where('job_id', $job_id)->first();
                if ($nostr_job) {

                    $logger->log('info', 'Found NostrJob: '.$job_id);

                    // update the model payload and content
                    // $nostr_job->payload = $payload;
                    $nostr_job->content = $content;
                    $nostr_job->save();

                    // Dispatch a job to the thread_id using websocket
                    NostrJobReady::dispatch($nostr_job);
                } else {
                    $logger->log('info', 'NostrJob not found: '.$job_id);
                }

                $this->processAgent($job_id);

                return [
                    'status' => 'success',
                    'message' => 'data processed',
                    'data' => $result,
                ];
            } else {
                $logger->log('info', 'Event with status '.$status.': '.json_encode($payload));

                return [
                    'status' => 'success',
                    'message' => 'data skipped',
                ];
            }
        }
    }

    public function processAgent($job_id)
    {
        $agentJob = AgentJob::where('job_id', $job_id)->first();

        if ($agentJob) {
            $agentJob->is_rag_ready = true;
            $agentJob->save();

            $agent = Agent::find($agentJob->agent_id);
            if ($agent) {
                if (! $agent->is_rag_ready) {
                    $agent->is_rag_ready = true;
                    $agent->save();
                }
                AgentRagReady::dispatch($agentJob);
            }
        }
    }
}
