<?php

namespace WorkOS\Resource;

/**
 * Class UserResponse.
 *
 * @property User $user
 */
class UserResponse extends BaseWorkOSResource
{
    public const RESOURCE_ATTRIBUTES = [
        "user"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [];

    public static function constructFromResponse($response)
    {
        $instance = parent::constructFromResponse($response);

        $instance->values["user"] = User::constructFromResponse($response["user"]);

        return $instance;
    }
}
