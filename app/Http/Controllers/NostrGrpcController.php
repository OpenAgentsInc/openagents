<?php

namespace App\Http\Controllers;

use App\Grpc\nostr\JobInput;
use App\Grpc\nostr\JobParam;
use App\Grpc\nostr\PoolConnectorClient;
use App\Grpc\nostr\RpcRequestJob;
use Exception;
use Grpc\ChannelCredentials;
use Illuminate\Http\Request;
use Illuminate\Routing\Controller;
use Illuminate\Support\Facades\Log;

class NostrGrpcController extends Controller
{
    public function requestContext($poolAddress, $query, $documents = [], $k = 1, $max_tokens = 512, $overlap = 128, $encryptFor = '')
    {
        $currentime = now();
        $expiresAt = $currentime->addMinutes(10);

        // Create a new instance of RpcRequestJob
        $requestJob = new RpcRequestJob();

        // Set the runOn field
        $requestJob->setRunOn('openagents/embeddings');

        // Set the expireAfter field (e.g., 10 minutes from now)
        $requestJob->setExpireAfter($expiresAt);

        $inputs = [];

        if ($documents != []) {

            // Set the input field
            foreach ($documents as $document) {
                $input = new JobInput();
                $input->setData($document);
                $input->setType('url');
                $input->setMarker('passage');
                $inputs[] = $input;
            }
        }

        $inputq = new JobInput();
        $inputq->setData($query);
        $inputq->setType('text');
        $inputq->setMarker('query');
        $inputs[] = $inputq;

        $requestJob->setInput($inputs);

        // Set the param field
        $param1 = new JobParam();
        $param1->setKey('max-tokens');
        $param1->setValue(["$max_tokens"]);

        $param2 = new JobParam();
        $param2->setKey('overlap');
        $param2->setValue(["$overlap"]);

        $param3 = new JobParam();
        $param3->setKey('quantize');
        $param3->setValue(['true']);

        $param4 = new JobParam();
        $param4->setKey('k');
        $param4->setValue(["$k"]);

        // Set the RepeatedField to the 'param' field of the requestJob message
        $requestJob->setParam([$param1, $param2, $param3, $param4]);

        // Set the description field
        $requestJob->setDescription('RAG pipeline');

        // Set the kind field (optional)
        $requestJob->setKind(5003);

        // Set the outputFormat field
        $requestJob->setOutputFormat('application/json');

        // encrypt for a specific provider
        if ($encryptFor != null) {
            $requestJob->setEncrypted(true);
            $requestJob->setRequestProvider($encryptFor);
        }

        $opts = [
            'credentials' => ChannelCredentials::createSsl(),
        ];
        $hostname = $poolAddress;
        $res = new PoolConnectorClient($hostname, $opts);
        $metadata = [];
        $options = [];
        $Jobres = $res->requestJob($requestJob, $metadata, $options);
        $result = $Jobres->wait();
        $status = $result[1]->code;
        if ($status !== 0) {
            throw new Exception($result[1]->details);
        }
        // get the thread_id and job_id to nostrJob model
        $job_id = $result[0]->id;

        // return $result;
        return $job_id;
    }

    public function handleJobRequest(Request $request)
    {

        // Create a new instance of RpcRequestJob
        $requestJob = new RpcRequestJob();

        // Set the runOn field
        $requestJob->setRunOn('openagents/embeddings');

        // Set the expireAfter field (e.g., 1 hour)
        $requestJob->setExpireAfter(3600);

        // Set the input field
        $input1 = new JobInput();
        $input1->setData('What is the color of the fox?');
        $input1->setType('text');
        $input1->setMarker('query');

        $input2 = new JobInput();
        $input2->setData('The quick brown fox jumps over the lazy dog.');
        $input1->setType('text');
        $input2->setMarker('passage');

        $requestJob->setInput([$input1, $input2]);

        // Set the param field
        $param1 = new JobParam();
        $param1->setKey('max-tokens');
        $param1->setValue(['512']);

        $param2 = new JobParam();
        $param2->setKey('overlap');
        $param2->setValue(['128']);

        $param3 = new JobParam();
        $param3->setKey('quantize');
        $param3->setValue(['true']);

        // Set the RepeatedField to the 'param' field of the requestJob message
        $requestJob->setParam([$param1, $param2, $param3]);

        // Set the description field
        $requestJob->setDescription('Embedding generation job');

        // Set the kind field (optional)
        $requestJob->setKind(5003);

        // Set the outputFormat field
        $requestJob->setOutputFormat('application/json');

        try {
            $opts = [
                'credentials' => ChannelCredentials::createSsl(),
            ];
            $hostname = 'openagents.forkforge.net:5000';
            $res = new PoolConnectorClient($hostname, $opts);
            // $response->sendSignedEvent($requestEvent);
            $metadata = [];
            $options = [];
            $Jobres = $res->requestJob($requestJob, $metadata, $options);
            $result = $Jobres->wait();
            $status = $result[1]->code;
            if ($status !== 0) {
                throw new Exception($result[1]->details);
            }
            // get the thread_id and job_id to nostrJob model
            $job_id = $result[0]->id;

            return $result;

            // return $result[1]->code;

        } catch (Exception $e) {
            Log::error($e);
        }
    }
}
