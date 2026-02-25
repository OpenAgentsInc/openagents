<?php

namespace WorkOS\Resource;

/**
 * Class OAuthTokens.
 *
 * @property string      $accessToken
 * @property string      $refreshToken
 * @property int         $expiresAt
 * @property array       $scopes
 */
class OAuthTokens extends BaseWorkOSResource
{
    public const RESOURCE_ATTRIBUTES = [
        "accessToken",
        "refreshToken",
        "expiresAt",
        "scopes"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "access_token" => "accessToken",
        "refresh_token" => "refreshToken",
        "expires_at" => "expiresAt",
        "scopes" => "scopes"
    ];

    public static function constructFromResponse($response)
    {
        $instance = parent::constructFromResponse($response);

        // Ensure scopes is always an array
        if (!isset($instance->values["scopes"])) {
            $instance->values["scopes"] = [];
        }

        return $instance;
    }
}
