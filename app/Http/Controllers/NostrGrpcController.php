<?php

namespace App\Http\Controllers;

use App\Grpc\nostr\JobInput;
use App\Grpc\nostr\JobParam;
use Illuminate\Http\Request;
use App\Grpc\nostr\RpcRequestJob;
use Illuminate\Routing\Controller;
use Illuminate\Support\Facades\Log;
use Google\Protobuf\Internal\GPBType;
use App\Grpc\nostr\PoolConnectorClient;
use Google\Protobuf\Internal\RepeatedField;
use App\Grpc\nostr\RpcSendSignedEventRequest;
use Grpc\Internal\InterceptorChannel;



class NostrGrpcController extends Controller
{
    private $poolConnectorClient;
    public function __construct()
    {
        $this->poolConnectorClient = PoolConnectorClient::class;
    }

    public function sendSignedEvent(Request $request)
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
        $input1->setMarker('query');

        $input2 = new JobInput();
        $input2->setData('The quick brown fox jumps over the lazy dog.');
        $input2->setMarker('passage');

        $requestJob->setInput(new RepeatedField(GPBType::MESSAGE, JobInput::class, [$input1, $input2]));

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

        $requestJob->setParam(new RepeatedField(GPBType::MESSAGE, JobParam::class, [$param1, $param2, $param3]));

        // Set the description field
        $requestJob->setDescription('Embedding generation job');

        // Set the kind field (optional)
        $requestJob->setKind(1);

        // Set the outputFormat field
        $requestJob->setOutputFormat('json');




        try {
            $opts = [
                'credentials' => ChannelCredentials::createInsecure(),
            ];
            $hostname = "openagents.forkforge.net:5000";
            $response = new poolConnectorClient($hostname, $opts, $channel = null);
            // $response->sendSignedEvent($requestEvent);
            $metadata = [];
            $options = [];
            $response->requestJob($requestJob);
            return $response;
            // Handle successful response (e.g., return success message)
        } catch (\Exception $e) {
            Log::error($e);
            // Handle gRPC errors (log or return error message)
        }
    }

    public function requestJob(Request $request)
    {

        // Create the gRPC request object (might be empty)
        $requestObject = new \RpcRequestJob();

        // Call requestJob on the client and handle response
        try {
            $response = $this->poolConnectorClient->requestJob($requestObject);
            // Handle successful response (e.g., return job data)
        } catch (\Exception $e) {
            // Handle gRPC errors (log or return error message)
        }
    }
}
