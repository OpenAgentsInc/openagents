<?php

namespace PostHog\Consumer;

use PostHog\HttpClient;
use PostHog\QueueConsumer;

class LibCurl extends QueueConsumer
{
    protected $type = "LibCurl";
    /**
     * @var HttpClient
     */
    private $httpClient;

    /**
     * Creates a new queued libcurl consumer
     * @param string $apiKey
     * @param array $options
     *     boolean  "debug" - whether to use debug output, wait for response.
     *     number   "max_queue_size" - the max size of messages to enqueue
     *     number   "batch_size" - how many messages to send in a single request
     */
    public function __construct($apiKey, $options = [], ?HttpClient $httpClient = null)
    {
        parent::__construct($apiKey, $options);
        $this->httpClient = $httpClient !== null ? $httpClient : new HttpClient(
            $this->host,
            $this->ssl(),
            $this->maximum_backoff_duration,
            $this->compress_request,
            $this->debug(),
            $this->options['error_handler'] ?? null
        );
    }

    /**
     * Define getter method for consumer type
     *
     * @return string
     */
    public function getConsumer()
    {
        return $this->type;
    }

    /**
     * Make a sync request to our API. If debug is
     * enabled, we wait for the response
     * and retry once to diminish impact on performance.
     * @param array $messages array of all the messages to send
     * @return boolean whether the request succeeded
     */
    public function flushBatch($messages)
    {
        $body = $this->payload($messages);
        $payload = json_encode($body);

        // Verify message size is below than 32KB
        if (strlen($payload) >= 32 * 1024) {
            if ($this->debug()) {
                $msg = "Message size is larger than 32KB";
                error_log("[PostHog][" . $this->type . "] " . $msg);
            }

            return false;
        }

        if ($this->compress_request) {
            $payload = gzencode($payload);
        }

        return $this->httpClient->sendRequest(
            '/batch/',
            $payload,
            [
                // Send user agent in the form of {library_name}/{library_version} as per RFC 7231.
                "User-Agent: {$messages[0]['library']}/{$messages[0]['library_version']}",
            ],
            [
                'shouldVerify' => $this->options['verify_batch_events_request'] ?? true,
            ]
        )->getResponse();
    }
}
