<?php

namespace WorkOS\Resource;

/**
 * Class AuthenticationChallenge.
 */
class VerificationChallenge extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "authentication_challenge";

    public const RESOURCE_ATTRIBUTES = [
        "challenge",
        "valid"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "challenge" => "challenge",
        "valid" => "valid"
    ];
}
