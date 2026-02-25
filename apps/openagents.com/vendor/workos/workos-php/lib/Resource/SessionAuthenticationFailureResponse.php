<?php

namespace WorkOS\Resource;

/**
 * Class SessionAuthenticationFailureResponse.
 *
 * Represents a failed session authentication.
 *
 * @property bool $authenticated
 * @property string $reason
 */
class SessionAuthenticationFailureResponse extends BaseWorkOSResource
{
    public const REASON_NO_SESSION_COOKIE_PROVIDED = "NO_SESSION_COOKIE_PROVIDED";
    public const REASON_INVALID_SESSION_COOKIE = "INVALID_SESSION_COOKIE";
    public const REASON_ENCRYPTION_ERROR = "ENCRYPTION_ERROR";
    public const REASON_HTTP_ERROR = "HTTP_ERROR";

    public const RESOURCE_ATTRIBUTES = [
        "authenticated",
        "reason"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "authenticated" => "authenticated",
        "reason" => "reason"
    ];

    /**
     * Construct a failure response with a specific reason.
     *
     * @param string $reason Reason for authentication failure
     */
    public function __construct(string $reason)
    {
        $this->values = [
            "authenticated" => false,
            "reason" => $reason
        ];
        $this->raw = [];
    }
}
