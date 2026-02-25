<?php

namespace PostHog;

use Closure;

class HttpClient
{
    /**
     * @var string
     */
    private $host;

    /**
     * @var bool
     */
    private $useSsl;

    /**
     * @var int
     */
    private $maximumBackoffDuration;

    /**
     * @var bool
     */
    private $compressRequests;

    /**
     * @var Closure|null
     */
    private $errorHandler;
    /**
     * @var bool
     */
    private $debug;

    /**
     * @var int The maximum number of milliseconds to allow cURL functions to execute / wait.
     */
    private $curlTimeoutMilliseconds;

    public function __construct(
        string $host,
        bool $useSsl = true,
        int $maximumBackoffDuration = 10000,
        bool $compressRequests = false,
        bool $debug = false,
        ?Closure $errorHandler = null,
        int $curlTimeoutMilliseconds = 10000
    ) {
        $this->host = $host;
        $this->useSsl = $useSsl;
        $this->maximumBackoffDuration = $maximumBackoffDuration;
        $this->compressRequests = $compressRequests;
        $this->debug = $debug;
        $this->errorHandler = $errorHandler;
        $this->curlTimeoutMilliseconds = $curlTimeoutMilliseconds;
    }

    /**
     * @param string $path
     * @param string|null $payload
     * @param array $extraHeaders
     * @param array $requestOptions
     * @return HttpResponse
     */
    public function sendRequest(string $path, ?string $payload, array $extraHeaders = [], array $requestOptions = []): HttpResponse
    {
        $protocol = $this->useSsl ? "https://" : "http://";

        $backoff = 100; // Set initial waiting time to 100ms

        $shouldRetry = $requestOptions['shouldRetry'] ?? true;
        $shouldVerify = $requestOptions['shouldVerify'] ?? true;
        $includeEtag = $requestOptions['includeEtag'] ?? false;

        do {
            // open connection
            $ch = curl_init();

            if (null !== $payload) {
                curl_setopt($ch, CURLOPT_POSTFIELDS, $payload);
            }

            $headers = [];
            $headers[] = 'Content-Type: application/json';
            if ($this->compressRequests) {
                $headers[] = 'Content-Encoding: gzip';
            }

            // check if timeout exists in request options, if not use default
            $timeout = $this->curlTimeoutMilliseconds;
            if (isset($requestOptions['timeout'])) {
                $timeout = $requestOptions['timeout'];
            }

            curl_setopt($ch, CURLOPT_HTTPHEADER, array_merge($headers, $extraHeaders));
            curl_setopt($ch, CURLOPT_URL, $protocol . $this->host . $path);
            curl_setopt($ch, CURLOPT_RETURNTRANSFER, $shouldVerify);
            curl_setopt($ch, CURLOPT_TIMEOUT_MS, $shouldVerify ? $timeout : 1);
            curl_setopt($ch, CURLOPT_CONNECTTIMEOUT_MS, $timeout);
            if (! $shouldVerify) {
                curl_setopt($ch, CURLOPT_NOSIGNAL, true);
                curl_setopt($ch, CURLOPT_FRESH_CONNECT, true);
            }

            // Capture response headers if we need to extract ETag
            if ($includeEtag) {
                curl_setopt($ch, CURLOPT_HEADER, true);
            }

            // retry failed requests just once to diminish impact on performance
            $httpResponse = $this->executePost($ch, $includeEtag);
            $responseCode = $httpResponse->getResponseCode();

            //close connection
            curl_close($ch);

            // Handle 304 Not Modified - this is a success, not an error
            if ($responseCode === 304) {
                if ($this->debug) {
                    $maskedUrl = $this->maskTokensInUrl($protocol . $this->host . $path);
                    error_log("[PostHog][HttpClient] GET " . $maskedUrl . " returned 304 Not Modified");
                }
                break;
            }

            if ($shouldVerify && 200 != $responseCode) {
                // log error
                $this->handleError($ch, $responseCode);

                if ($shouldRetry === false) {
                    break;
                } elseif (($responseCode >= 500 && $responseCode <= 600) || 429 == $responseCode) {
                    // If status code is greater than 500 and less than 600, it indicates server error
                    // Error code 429 indicates rate limited.
                    // Retry uploading in these cases.
                    usleep($backoff * 1000);
                    $backoff *= 2;
                } elseif ($responseCode >= 400) {
                    break;
                } elseif ($responseCode == 0) {
                    break;
                }
            } else {
                break;  // no error
            }
        } while ($shouldRetry && $backoff < $this->maximumBackoffDuration);

        return $httpResponse;
    }

    private function executePost($ch, bool $includeEtag = false): HttpResponse
    {
        $response = curl_exec($ch);
        $responseCode = curl_getinfo($ch, CURLINFO_RESPONSE_CODE);
        $etag = null;

        if ($includeEtag && $response !== false) {
            // Parse headers to extract ETag
            $headerSize = curl_getinfo($ch, CURLINFO_HEADER_SIZE);
            $headers = substr($response, 0, $headerSize);
            $body = substr($response, $headerSize);

            // Extract ETag from headers (case-insensitive)
            if (preg_match('/^etag:\s*(.+)$/mi', $headers, $matches)) {
                $etag = trim($matches[1]);
            }

            return new HttpResponse($body, $responseCode, $etag);
        }

        return new HttpResponse($response, $responseCode);
    }

    private function handleError($code, $message)
    {
        if (null !== $this->errorHandler) {
            $handler = $this->errorHandler;
            $handler($code, $message);
        }

        if ($this->debug) {
            error_log("[PostHog][HttpClient] " . $message);
        }
    }

    /**
     * Mask tokens in URLs to avoid exposing them in logs
     *
     * @param string $url
     * @return string
     */
    public function maskTokensInUrl(string $url): string
    {
        return preg_replace('/token=[^&]+/', 'token=[REDACTED]', $url);
    }
}
