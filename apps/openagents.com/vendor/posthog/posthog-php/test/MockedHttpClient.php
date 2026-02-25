<?php

namespace PostHog\Test;

use Closure;
use PostHog\HttpResponse;
use PostHog\Test\Assets\MockedResponses;

class MockedHttpClient extends \PostHog\HttpClient
{
    public $calls;

    private $flagEndpointResponse;
    private $flagsEndpointResponse;
    private $flagEndpointEtag;
    private $flagEndpointResponseCode;

    /** @var array|null Queue of responses for sequential calls */
    private $flagEndpointResponseQueue;

    public function __construct(
        string $host,
        bool $useSsl = true,
        int $maximumBackoffDuration = 10000,
        bool $compressRequests = false,
        bool $debug = false,
        ?Closure $errorHandler = null,
        int $curlTimeoutMilliseconds = 750,
        array $flagEndpointResponse = [],
        array $flagsEndpointResponse = [],
        ?string $flagEndpointEtag = null,
        int $flagEndpointResponseCode = 200
    ) {
        parent::__construct(
            $host,
            $useSsl,
            $maximumBackoffDuration,
            $compressRequests,
            $debug,
            $errorHandler,
            $curlTimeoutMilliseconds
        );
        $this->flagEndpointResponse = $flagEndpointResponse;
        $this->flagsEndpointResponse = !empty($flagsEndpointResponse) ? $flagsEndpointResponse : MockedResponses::FLAGS_REQUEST;
        $this->flagEndpointEtag = $flagEndpointEtag;
        $this->flagEndpointResponseCode = $flagEndpointResponseCode;
        $this->flagEndpointResponseQueue = null;
    }

    /**
     * Set a queue of responses for the local_evaluation endpoint
     * Each call will consume the next response in the queue
     *
     * @param array $responses Array of ['response' => array, 'etag' => string|null, 'responseCode' => int]
     */
    public function setFlagEndpointResponseQueue(array $responses): void
    {
        $this->flagEndpointResponseQueue = $responses;
    }

    public function sendRequest(string $path, ?string $payload, array $extraHeaders = [], array $requestOptions = []): HttpResponse
    {
        if (!isset($this->calls)) {
            $this->calls = [];
        }
        array_push($this->calls, array("path" => $path, "payload" => $payload, "extraHeaders" => $extraHeaders, "requestOptions" => $requestOptions));

        if (str_starts_with($path, "/flags/")) {
            return new HttpResponse(json_encode($this->flagsEndpointResponse), 200);
        }

        if (str_starts_with($path, "/api/feature_flag/local_evaluation")) {
            // Check if we have a response queue
            if ($this->flagEndpointResponseQueue !== null && !empty($this->flagEndpointResponseQueue)) {
                $nextResponse = array_shift($this->flagEndpointResponseQueue);
                $response = $nextResponse['response'] ?? [];
                $etag = $nextResponse['etag'] ?? null;
                $responseCode = $nextResponse['responseCode'] ?? 200;

                // Handle 304 Not Modified - return empty body
                if ($responseCode === 304) {
                    return new HttpResponse('', $responseCode, $etag);
                }

                return new HttpResponse(json_encode($response), $responseCode, $etag);
            }

            // Handle 304 Not Modified - return empty body
            if ($this->flagEndpointResponseCode === 304) {
                return new HttpResponse('', 304, $this->flagEndpointEtag);
            }

            return new HttpResponse(
                json_encode($this->flagEndpointResponse),
                $this->flagEndpointResponseCode,
                $this->flagEndpointEtag
            );
        }

        return parent::sendRequest($path, $payload, $extraHeaders, $requestOptions);
    }
}
