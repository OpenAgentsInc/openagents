<?php

namespace App\Http\Controllers\Webhook;

use App\Http\Controllers\Controller;
use App\Jobs\JobResultReceiverJob;
use App\Services\OpenObserveLogger;
use Illuminate\Http\Request;

class PoolWebhookReceiver extends Controller
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

        if (config('nostr.webhook_secret') && $secret !== config('nostr.webhook_secret')) {
            return response()->json(['error' => 'Invalid token'], 403);
        }

        if ($data[0] == 'Job') { // Handle job Event
            $payload = $data[1];
            $status = $payload['state']['status'];

            if ($status == 2) {
                $logger->log('error', 'Event with status 2: '.json_encode($payload));
                $logger->close();
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

                JobResultReceiverJob::dispatch($job_id, $content, $payload);
                $logger->close();
                return [
                    'status' => 'success',
                    'message' => 'data processed',
                    'data' => $result,
                ];
            } else {
                $logger->log('info', 'Event with status '.$status.': '.json_encode($payload));
                $logger->close();
                return [
                    'status' => 'success',
                    'message' => 'data skipped',
                ];
            }
        } else { // Handle Event Event
            // do nothing
        }
    }
}
