<?php

namespace WorkOS\Resource;

/**
 * Class Response.
 *
 * Representation of response resulting from a Client request.
 */
class Response
{
    /**
     * @var string $body
     */
    public $body;

    /**
     * @var array $headers
     */
    public $headers;

    /**
     * @var array $json
     */
    public $json;

    /**
     * @var int @statusCode
     */
    public $statusCode;

    public function __construct($body, $headers, $statusCode)
    {
        $this->body = $body;
        $this->headers = $headers;
        $this->statusCode = $statusCode;
    }

    public function json()
    {
        if (!isset($json)) {
            $this->json = \json_decode($this->body, true);
        }

        return $this->json;
    }
}
