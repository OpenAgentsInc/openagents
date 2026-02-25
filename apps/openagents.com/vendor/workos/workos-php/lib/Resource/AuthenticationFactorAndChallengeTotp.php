<?php

namespace WorkOS\Resource;

/**
 * Class AuthenticationFactorAndChallengeTotp.
 *
 * @property AuthenticationFactorTotp $authenticationFactor
 * @property AuthenticationChallengeTotp $authenticationChallenge
 */
class AuthenticationFactorAndChallengeTotp extends BaseWorkOSResource
{
    public const RESOURCE_ATTRIBUTES = [
        "authenticationFactor",
        "authenticationChallenge"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "authentication_factor" => "authenticationFactor",
        "authentication_challenge" => "authenticationChallenge"
    ];

    public static function constructFromResponse($response)
    {
        $instance = parent::constructFromResponse($response);
        $instance->values["authenticationFactor"] = AuthenticationFactorTotp::constructFromResponse($response["authentication_factor"]);
        $instance->values["authenticationChallenge"] = AuthenticationChallengeTotp::constructFromResponse($response["authentication_challenge"]);

        return $instance;
    }
}
