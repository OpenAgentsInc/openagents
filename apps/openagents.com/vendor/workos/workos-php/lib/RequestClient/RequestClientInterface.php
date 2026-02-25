<?php

namespace WorkOS\RequestClient;

use WorkOS\Exception\GenericException;

/**
 * Interface RequestClientInterface.
 */
interface RequestClientInterface
{
    /**
     * @param string $method Client method
     * @param string $url URL to resource
     * @param null|array $headers Headers for request
     * @param null|array $params Associative array that'll be passed as query parameters or form data
     *
     * @throws GenericException if a client level exception is encountered
     *
     * @return array An array composed of the result string, response headers and status code
     */
    public function request($method, $url, ?array $headers = null, ?array $params = null);
}
