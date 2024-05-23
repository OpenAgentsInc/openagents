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

        ]);

        $data = $request->all();
        $secret = $request->query('secret');

        if (config('nostr.webhook_secret') && $secret !== config('nostr.webhook_secret')) {
            return response()->json(['error' => 'Invalid token'], 403);
        }

        if ($data[0] == 'Job') { // Handle job Event
            $payload = $data[1];
            $logger->log('info', 'Received Job Update' . json_encode($payload));

            $states = $payload['results'];
            foreach ($states as $state) {
                $status = $state['status'];
                if($status == 3){
                    $job_id = $payload['id'];
                    $content = $payload['result']['content'];
                }
                $job_id = $state['id'];
                $content = $state['result']["content"];
                JobResultReceiverJob::dispatch($job_id, $content, $payload);
                break;
            }

            return [
                'status' => 'success',
                'message' => 'received',
            ];
        }
    }
}
