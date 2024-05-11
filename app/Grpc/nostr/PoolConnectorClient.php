<?php
// GENERATED CODE -- DO NOT EDIT!

namespace App\Grpc\nostr;

/**
 */
class PoolConnectorClient extends \Grpc\BaseStub {

    /**
     * @param string $hostname hostname
     * @param array $opts channel options
     * @param \Grpc\Channel $channel (optional) re-use channel object
     */
    public function __construct($hostname, $opts, $channel = null) {
        parent::__construct($hostname, $opts, $channel);
    }

    /**
     * job management
     * @param \App\Grpc\nostr\RpcRequestJob $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function requestJob(\App\Grpc\nostr\RpcRequestJob $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/requestJob',
        $argument,
        ['\App\Grpc\nostr\Job', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcJobRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function sendJobRequest(\App\Grpc\nostr\RpcJobRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/sendJobRequest',
        $argument,
        ['\App\Grpc\nostr\Job', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcGetJob $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function getJob(\App\Grpc\nostr\RpcGetJob $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/getJob',
        $argument,
        ['\App\Grpc\nostr\Job', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcGetPendingJobs $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function getPendingJobs(\App\Grpc\nostr\RpcGetPendingJobs $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/getPendingJobs',
        $argument,
        ['\App\Grpc\nostr\PendingJobs', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcGetJob $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function isJobDone(\App\Grpc\nostr\RpcGetJob $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/isJobDone',
        $argument,
        ['\App\Grpc\nostr\RpcIsJobDone', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcAcceptJob $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function acceptJob(\App\Grpc\nostr\RpcAcceptJob $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/acceptJob',
        $argument,
        ['\App\Grpc\nostr\Job', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcCancelJob $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function cancelJob(\App\Grpc\nostr\RpcCancelJob $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/cancelJob',
        $argument,
        ['\App\Grpc\nostr\Job', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcJobOutput $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function outputForJob(\App\Grpc\nostr\RpcJobOutput $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/outputForJob',
        $argument,
        ['\App\Grpc\nostr\Job', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcJobComplete $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function completeJob(\App\Grpc\nostr\RpcJobComplete $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/completeJob',
        $argument,
        ['\App\Grpc\nostr\Job', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcJobLog $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function logForJob(\App\Grpc\nostr\RpcJobLog $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/logForJob',
        $argument,
        ['\App\Grpc\nostr\Job', 'decode'],
        $metadata, $options);
    }

    /**
     * discovery
     * @param \App\Grpc\nostr\RpcAnnounceNodeRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function announceNode(\App\Grpc\nostr\RpcAnnounceNodeRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/announceNode',
        $argument,
        ['\App\Grpc\nostr\RpcAnnounceNodeResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcAnnounceTemplateRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function announceEventTemplate(\App\Grpc\nostr\RpcAnnounceTemplateRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/announceEventTemplate',
        $argument,
        ['\App\Grpc\nostr\RpcAnnounceTemplateResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcDiscoverPoolsRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function discoverPools(\App\Grpc\nostr\RpcDiscoverPoolsRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/discoverPools',
        $argument,
        ['\App\Grpc\nostr\RpcDiscoverPoolsResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcDiscoverNodesRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function discoverNodes(\App\Grpc\nostr\RpcDiscoverNodesRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/discoverNodes',
        $argument,
        ['\App\Grpc\nostr\RpcDiscoverNodesResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcDiscoverActionsRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function discoverActions(\App\Grpc\nostr\RpcDiscoverActionsRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/discoverActions',
        $argument,
        ['\App\Grpc\nostr\RpcDiscoverActionsResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcDiscoverNearbyNodesRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function discoverNearbyNodes(\App\Grpc\nostr\RpcDiscoverNearbyNodesRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/discoverNearbyNodes',
        $argument,
        ['\App\Grpc\nostr\RpcDiscoverNearbyNodesResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcDiscoverNearbyActionsRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function discoverNearbyActions(\App\Grpc\nostr\RpcDiscoverNearbyActionsRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/discoverNearbyActions',
        $argument,
        ['\App\Grpc\nostr\RpcDiscoverNearbyActionsResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * generic nostr events
     * @param \App\Grpc\nostr\RpcSendSignedEventRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function sendSignedEvent(\App\Grpc\nostr\RpcSendSignedEventRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/sendSignedEvent',
        $argument,
        ['\App\Grpc\nostr\RpcSendSignedEventResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcSubscribeToEventsRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function subscribeToEvents(\App\Grpc\nostr\RpcSubscribeToEventsRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/subscribeToEvents',
        $argument,
        ['\App\Grpc\nostr\RpcSubscribeToEventsResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcUnsubscribeFromEventsRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function unsubscribeFromEvents(\App\Grpc\nostr\RpcUnsubscribeFromEventsRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/unsubscribeFromEvents',
        $argument,
        ['\App\Grpc\nostr\RpcUnsubscribeFromEventsResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcGetEventsRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function getEvents(\App\Grpc\nostr\RpcGetEventsRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/getEvents',
        $argument,
        ['\App\Grpc\nostr\RpcGetEventsResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * blob storage
     * @param \App\Grpc\nostr\RpcCreateDiskRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function createDisk(\App\Grpc\nostr\RpcCreateDiskRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/createDisk',
        $argument,
        ['\App\Grpc\nostr\RpcCreateDiskResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcOpenDiskRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function openDisk(\App\Grpc\nostr\RpcOpenDiskRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/openDisk',
        $argument,
        ['\App\Grpc\nostr\RpcOpenDiskResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcCloseDiskRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function closeDisk(\App\Grpc\nostr\RpcCloseDiskRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/closeDisk',
        $argument,
        ['\App\Grpc\nostr\RpcCloseDiskResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcDiskDeleteFileRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function diskDeleteFile(\App\Grpc\nostr\RpcDiskDeleteFileRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/diskDeleteFile',
        $argument,
        ['\App\Grpc\nostr\RpcDiskDeleteFileResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcDiskListFilesRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function diskListFiles(\App\Grpc\nostr\RpcDiskListFilesRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/diskListFiles',
        $argument,
        ['\App\Grpc\nostr\RpcDiskListFilesResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcDiskReadFileRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\ServerStreamingCall
     */
    public function diskReadFile(\App\Grpc\nostr\RpcDiskReadFileRequest $argument,
      $metadata = [], $options = []) {
        return $this->_serverStreamRequest('/PoolConnector/diskReadFile',
        $argument,
        ['\App\Grpc\nostr\RpcDiskReadFileResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcDiskReadFileRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function diskReadSmallFile(\App\Grpc\nostr\RpcDiskReadFileRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/diskReadSmallFile',
        $argument,
        ['\App\Grpc\nostr\RpcDiskReadFileResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\ClientStreamingCall
     */
    public function diskWriteFile($metadata = [], $options = []) {
        return $this->_clientStreamRequest('/PoolConnector/diskWriteFile',
        ['\App\Grpc\nostr\RpcDiskWriteFileResponse','decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcDiskWriteFileRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\UnaryCall
     */
    public function diskWriteSmallFile(\App\Grpc\nostr\RpcDiskWriteFileRequest $argument,
      $metadata = [], $options = []) {
        return $this->_simpleRequest('/PoolConnector/diskWriteSmallFile',
        $argument,
        ['\App\Grpc\nostr\RpcDiskWriteFileResponse', 'decode'],
        $metadata, $options);
    }

    /**
     * cache
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\ClientStreamingCall
     */
    public function cacheSet($metadata = [], $options = []) {
        return $this->_clientStreamRequest('/PoolConnector/cacheSet',
        ['\App\Grpc\nostr\RpcCacheSetResponse','decode'],
        $metadata, $options);
    }

    /**
     * @param \App\Grpc\nostr\RpcCacheGetRequest $argument input argument
     * @param array $metadata metadata
     * @param array $options call options
     * @return \Grpc\ServerStreamingCall
     */
    public function cacheGet(\App\Grpc\nostr\RpcCacheGetRequest $argument,
      $metadata = [], $options = []) {
        return $this->_serverStreamRequest('/PoolConnector/cacheGet',
        $argument,
        ['\App\Grpc\nostr\RpcCacheGetResponse', 'decode'],
        $metadata, $options);
    }

}
