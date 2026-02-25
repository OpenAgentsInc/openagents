<?php

namespace PostHog\Consumer;

use Exception;
use PostHog\QueueConsumer;

class Socket extends QueueConsumer
{
    protected $type = "Socket";
    private $socket_failed;

    /**
     * Creates a new socket consumer for dispatching async requests immediately
     * @param string $apiKey
     * @param array $options
     *     number   "timeout" - the timeout for connecting
     *     function "error_handler" - function called back on errors.
     *     boolean  "debug" - whether to use debug output, wait for response.
     */
    public function __construct($apiKey, $options = array())
    {
        if (!isset($options["timeout"])) {
            $options["timeout"] = 5;
        }

        parent::__construct($apiKey, $options);
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

    public function flushBatch($batch)
    {
        $socket = $this->createSocket();

        if (!$socket) {
            return;
        }

        $payload = $this->payload($batch);
        $payload = json_encode($payload);

        $body = $this->createBody($this->host, $payload);
        if (false === $body) {
            return false;
        }

        return $this->makeRequest($socket, $body);
    }

    private function createSocket()
    {
        if ($this->socket_failed) {
            return false;
        }

        $protocol = $this->ssl() ? "ssl" : "tcp";
        $port = $this->ssl() ? 443 : 80;
        $timeout = $this->options["timeout"];

        try {
            // Open our socket to the API Server.
            // Since we're try catch'ing prevent PHP logs.
            $socket = @pfsockopen(
                $protocol . "://" . $this->host,
                $port,
                $errno,
                $errstr,
                $timeout
            );

            // If we couldn't open the socket, handle the error.
            if (false === $socket) {
                $this->handleError($errno, $errstr);
                $this->socket_failed = true;

                return false;
            }

            return $socket;
        } catch (Exception $e) {
            $this->handleError($e->getCode(), $e->getMessage());
            $this->socket_failed = true;

            return false;
        }
    }

    /**
     * Attempt to write the request to the socket, wait for response if debug
     * mode is enabled.
     * @param resource $socket the handle for the socket
     * @param string $req request body
     * @param boolean $retry
     * @return boolean $success
     */
    private function makeRequest($socket, $req, $retry = true)
    {
        $bytes_written = 0;
        $bytes_total = strlen($req);
        $closed = false;

        // Retries with exponential backoff until success
        $backoff = 100;   // Set initial waiting time to 100ms

        while (true) {
            // Send request to server
            while (!$closed && $bytes_written < $bytes_total) {
                try {
                    // Since we're try catch'ing prevent PHP logs.
                    $written = @fwrite($socket, substr($req, $bytes_written));
                } catch (Exception $e) {
                    $this->handleError($e->getCode(), $e->getMessage());
                    $closed = true;
                }
                if (!isset($written) || !$written) {
                    $closed = true;
                } else {
                    $bytes_written += $written;
                }
            }

            // Get response for request
            $statusCode = 0;
            $errorMessage = "";

            if (!$closed) {
                $res = $this->parseResponse(fread($socket, 2048));
                $statusCode = (int)$res["status"];
                $errorMessage = $res["message"];
            }
            fclose($socket);

            // If status code is 200, return true
            if (200 == $statusCode) {
                return true;
            }

            // If status code is greater than 500 and less than 600, it indicates server error
            // Error code 429 indicates rate limited.
            // Retry uploading in these cases.
            if (($statusCode >= 500 && $statusCode <= 600) || 429 == $statusCode || 0 == $statusCode) {
                if ($backoff >= $this->maximum_backoff_duration) {
                    break;
                }

                usleep($backoff * 1000);
            } elseif ($statusCode >= 400) {
                if ($this->debug()) {
                    $this->handleError($res["status"], $res["message"]);
                }

                break;
            }

            // Retry uploading...
            $backoff *= 2;
            $socket = $this->createSocket();
        }

        return true;
    }

    /**
     * Create the body to send as the post request.
     * @param string $host
     * @param string $content
     * @return string body
     */
    private function createBody($host, $content)
    {
        $req = "";
        $req .= "POST /batch/ HTTP/1.1\r\n";
        $req .= "Host: " . $host . "\r\n";
        $req .= "Content-Type: application/json\r\n";
        $req .= "Accept: application/json\r\n";

        // Send user agent in the form of {library_name}/{library_version} as per RFC 7231.
        $content_json = json_decode($content, true);
        $libName = $content_json['batch'][0]['library'];
        $libVersion = $content_json['batch'][0]['library_version'];
        $req .= "User-Agent: $libName/$libVersion\r\n";

        // Compress content if compress_request is true
        if ($this->compress_request) {
            $content = gzencode($content);

            $req .= "Content-Encoding: gzip\r\n";
        }

        $req .= "Content-length: " . strlen($content) . "\r\n";
        $req .= "\r\n";
        $req .= $content;

        // Verify message size is below than 32KB
        if (strlen($req) >= 32 * 1024) {
            if ($this->debug()) {
                $msg = "Message size is larger than 32KB";
                error_log("[PostHog][" . $this->type . "] " . $msg);
            }

            return false;
        }

        return $req;
    }

    /**
     * Parse our response from the server, check header and body.
     * @param string $res
     * @return array
     *     string $status  HTTP code, e.g. "200"
     *     string $message JSON response from the api
     */
    private function parseResponse($res)
    {
        $contents = explode("\n", $res);

        // Response comes back as HTTP/1.1 200 OK
        // Final line contains HTTP response.
        $status = explode(" ", $contents[0], 3);
        $result = $contents[count($contents) - 1];

        return array(
            "status" => isset($status[1]) ? $status[1] : null,
            "message" => $result
        );
    }
}
