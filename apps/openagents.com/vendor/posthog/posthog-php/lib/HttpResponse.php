<?php

namespace PostHog;

class HttpResponse
{
    private $response;
    private $responseCode;
    private $etag;

    public function __construct($response, $responseCode, ?string $etag = null)
    {
        $this->response = $response;
        $this->responseCode = $responseCode;
        $this->etag = $etag;
    }

    /**
     * @return mixed
     */
    public function getResponse()
    {
        return $this->response;
    }

    /**
     * @return mixed
     */
    public function getResponseCode()
    {
        return $this->responseCode;
    }

    /**
     * @return string|null
     */
    public function getEtag(): ?string
    {
        return $this->etag;
    }

    /**
     * Check if the response is a 304 Not Modified
     *
     * @return bool
     */
    public function isNotModified(): bool
    {
        return $this->responseCode === 304;
    }
}
