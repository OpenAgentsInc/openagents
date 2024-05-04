<?php

namespace App\Http\Controllers\Webhook;

use App\Models\AgentJob;
use App\Models\NostrJob;
use Illuminate\Http\Request;
use App\Events\AgentRagReady;
use App\Events\NostrJobReady;
use App\Services\OpenObserveLogger;
use App\Http\Controllers\Controller;

class NostrHandlerController extends Controller
{
    public function handleEvent(Request $request)
    {
        $data = $request->all();

        $logger = new OpenObserveLogger([
            'baseUrl' => 'https://pool.openagents.com:5080',
            'org' => 'default',
            'stream' => 'logs',
            'batchSize' => 1,
            'flushInterval' => 1000,
        ]);
        $logger->log('info', 'Event received');
        $logger->log('info', json_encode($data));

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
                // log the error
                //                Log::error($data);
                //                Log::channel('slack')->error(json_encode($extractedData));

                return [
                    'status' => 'success',
                    'message' => 'error logged',
                ];
            } elseif ($status == 3) {

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
                    // update the model payload and content
                    $nostr_job->payload = $payload;
                    $nostr_job->content = $content;
                    $nostr_job->save();

                    // Dispatch a job to the thread_id using websocket
                    NostrJobReady::dispatch($nostr_job);
                }

                $this->ProcessAgent($job_id);

                return [
                    'status' => 'success',
                    'message' => 'data processed',
                    'data' => $result,
                ];
            } else {
                return [
                    'status' => 'success',
                    'message' => 'data skipped',
                ];
            }
        }
    }


    public function ProcessAgent($job_id){
        $job = AgentJob::where('job_id',$job_id)->first();

        if($job){
            $job->is_rag_ready = true;
            $job->save();

            $agent = Agent::find($job->agent_id);
            $agent->is_rag_ready = true;
            $agent->save();


            AgentRagReady::dispatch($job);

        }
    }
}
