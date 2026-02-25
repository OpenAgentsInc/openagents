<?php

namespace WorkOS;

/**
 * Class Client.
 *
 * Communicate with the WorkOS API.
 */
class Client
{
    public const METHOD_GET = "get";
    public const METHOD_POST = "post";
    public const METHOD_DELETE = "delete";
    public const METHOD_PUT = "put";

    private static $_requestClient;

    public static function requestClient()
    {
        if (!self::$_requestClient) {
            self::$_requestClient = new RequestClient\CurlRequestClient();
        }

        return self::$_requestClient;
    }

    public static function setRequestClient($requestClient)
    {
        self::$_requestClient = $requestClient;
    }

    /**
     * @param string $method Client method
     * @param string $path Path to the WorkOS resource
     * @param null|array $headers Associative array containing headers
     * @param null|array $params Associative array that'll be passed as query parameters or form data
     * @param null|string $token Token to be included in request for authorization
     *
     * @throws \WorkOS\Exception\GenericException if a client level exception is encountered
     * @throws \WorkOS\Exception\ServerException if a 5xx status code is returned
     * @throws \WorkOS\Exception\AuthenticationException if a 401 status code is returned
     * @throws \WorkOS\Exception\AuthorizationException if a 403 status code is returned
     * @throws \WorkOS\Exception\NotFoundException if a 404 status code is returned
     * @throws \WorkOS\Exception\BadRequestException if a 400 status code is returned
     *
     * @return array<string, mixed>
     */
    public static function request($method, $path, ?array $headers = null, ?array $params = null, $withAuth = false)
    {
        $url = self::generateUrl($path);

        $requestHeaders = self::generateBaseHeaders($withAuth);
        if ($headers) {
            $requestHeaders = \array_merge($requestHeaders, $headers);
        }

        list($result, $responseHeaders, $responseCode) = self::requestClient()->request(
            $method,
            $url,
            $requestHeaders,
            $params
        );
        $response = new Resource\Response($result, $responseHeaders, $responseCode);

        if ($responseCode >= 400) {
            if ($responseCode >= 500) {
                throw new Exception\ServerException($response);
            } elseif ($responseCode === 401) {
                throw new Exception\AuthenticationException($response);
            } elseif ($responseCode === 403) {
                throw new Exception\AuthorizationException($response);
            } elseif ($responseCode === 404) {
                throw new Exception\NotFoundException($response);
            }

            throw new Exception\BadRequestException($response);
        }

        return $response->json();
    }

    /**
     * Generate base headers for request.
     *
     * @param boolean $withAuth return with authorization header if true
     *
     * @return array
     */
    public static function generateBaseHeaders($withAuth = false)
    {
        $baseHeaders = ["User-Agent: " . WorkOS::getIdentifier() . "/" . WORKOS::getVersion()];
        if ($withAuth) {
            \array_push($baseHeaders, "Authorization: Bearer " . WorkOS::getApikey());
        }

        return $baseHeaders;
    }

    /**
     * Generates a URL to the WorkOS API.
     *
     * @param string $path Path to the WorkOS resource
     * @param null|array $params Associative array to be passed as query parameters
     *
     * @return string
     */
    public static function generateUrl($path, ?array $params = null)
    {
        $url = WorkOS::getApiBaseUrl() . $path;

        if (is_array($params) && !empty($params)) {
            $queryParams = http_build_query($params);
            $url .= "?" . $queryParams;
        }

        return $url;
    }
}
