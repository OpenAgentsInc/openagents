<?php

namespace App\Http\Controllers\Webhook;

use App\Events\NostrJobReady;
use App\Http\Controllers\Controller;
use App\Models\NostrJob;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\Log;

class NostrHandlerController extends Controller
{
    public function handleEvent(Request $request)
    {
        $data = $request->all();

        // Log::info("$job");

        $requestType = $data[0];

        if ($requestType == 'Job') {
            $payload = $data[1];
            $status = $payload['state']['status'];
            if ($status == 2) {
                // log the error
                Log::error($data);

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
                    NostrJobReady::dispatch($nostr_job->id);
                }

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
}
