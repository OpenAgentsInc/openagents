<?php

namespace WorkOS\Resource;

/**
 * Class UserAndToken.
 *
 * @property string  $token
 * @property User $user
 */
class UserAndToken extends BaseWorkOSResource
{
    public const RESOURCE_ATTRIBUTES = [
        "token",
        "user"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "token" => "token"
    ];

    public static function constructFromResponse($response)
    {
        $instance = parent::constructFromResponse($response);

        $instance->values["user"] = User::constructFromResponse($response["user"]);

        return $instance;
    }
}
