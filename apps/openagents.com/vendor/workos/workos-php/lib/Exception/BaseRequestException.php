<?php

namespace WorkOS\Exception;

use WorkOS\Resource\Response;

/**
 * Class BaseRequestException.
 *
 * Base Exception for use in filtering a response for information.
 */
class BaseRequestException extends \Exception implements WorkOSException
{
    public $requestId = "";
    public $responseError;
    public $responseErrorDescription;
    public $responseErrors;
    public $responseCode;
    public $responseMessage;
    public $response;

    /**
     * BaseRequestException constructor.
     *
     * @param Response $response
     * @param null|string $message Exception message
     */
    public function __construct($response, ?string $message = null)
    {
        $this->response = $response;

        $responseJson = $response->json();

        if (!empty($responseJson["error"])) {
            $this->responseError = $responseJson["error"];
        }
        if (!empty($responseJson["error_description"])) {
            $this->responseErrorDescription = $responseJson["error_description"];
        }
        if (!empty($responseJson["errors"])) {
            $this->responseErrors = $responseJson["errors"];
        }
        if (!empty($responseJson["code"])) {
            $this->responseCode = $responseJson["code"];
        }
        if (!empty($responseJson["message"])) {
            $this->responseMessage = $responseJson["message"];
        }

        $this->filterResponseForException($response);

        if (isset($message)) {
            $this->message = $message;
        }
    }

    private function filterResponseForException($response)
    {
        try {
            $responseBody = $response->body;

            $this->message = $responseBody;
        } catch (\Exception $e) {
            $this->message = "";
        }

        if (\array_key_exists("x-request-id", $response->headers)) {
            $this->requestId = $response->headers["x-request-id"];
        }
    }
}
