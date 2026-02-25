<?php

namespace WorkOS\RequestClient;

use WorkOS\Client;
use WorkOS\Exception\GenericException;

/**
 * Class CurlRequestClient.
 */
class CurlRequestClient implements RequestClientInterface
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
    public function request($method, $url, ?array $headers = null, ?array $params = null)
    {
        if (empty($headers)) {
            $headers = array();
        }

        $opts = [\CURLOPT_RETURNTRANSFER => 1];

        switch ($method) {
            case Client::METHOD_GET:
                if (!empty($params)) {
                    $url .= "?" . http_build_query($params);
                }

                break;

            case Client::METHOD_POST:
                \array_push($headers, "Content-Type: application/json");

                $opts[\CURLOPT_POST] = 1;

                if (!empty($params)) {
                    $opts[\CURLOPT_POSTFIELDS] = \json_encode($params);
                }

                break;

            case Client::METHOD_DELETE:

                $opts[\CURLOPT_CUSTOMREQUEST] = 'DELETE';

                if (!empty($params)) {
                    $encoded = Util\Util::encodeParameters($params);
                    $absUrl = "{$absUrl}?{$encoded}";
                }

                break;

            case Client::METHOD_PUT:

                \array_push($headers, "Content-Type: application/json");

                $opts[\CURLOPT_CUSTOMREQUEST] = 'PUT';

                $opts[\CURLOPT_POST] = 1;

                if (!empty($params)) {
                    $opts[\CURLOPT_POSTFIELDS] = \json_encode($params);
                }

                break;
        }

        $opts[\CURLOPT_HTTPHEADER] = $headers;
        $opts[\CURLOPT_URL] = $url;

        return self::execute($opts);
    }

    private function execute($opts)
    {
        $curl = \curl_init();

        $headers = array();
        $headerCallback = function ($curl, $header_line) use (&$headers) {
            if (false === \strpos($header_line, ":")) {
                return \strlen($header_line);
            }

            list($key, $value) = \explode(":", \trim($header_line), 2);
            $headers[\trim($key)] = \trim($value);

            return \strlen($header_line);
        };
        $opts[\CURLOPT_HEADERFUNCTION] = $headerCallback;
        \curl_setopt_array($curl, $opts);

        $result = \curl_exec($curl);

        // I think this is for some sort of internal error
        // Any kind of response that returns a status code won"t hit this block
        if ($result === false) {
            $errno = \curl_errno($curl);
            $msg = \curl_error($curl);
            \curl_close($curl);

            throw new GenericException($msg, ["curlErrno" => $errno]);
        } else {
            // Unsure how versions of cURL and PHP correlate so using the legacy
            // reference for getting the last response code
            $statusCode = \curl_getinfo($curl, \CURLINFO_RESPONSE_CODE);
            \curl_close($curl);

            return [$result, $headers, $statusCode];
        }
    }
}
