<?php

namespace WorkOS\Resource;

/**
 * Class AuthenticationResponse.
 *
 * @property User $user
 * @property ?string $organizationId
 * @property string $accessToken
 * @property string $refreshToken
 * @property ?Impersonator $impersonator
 * @property ?OAuthTokens $oauthTokens
 */
class AuthenticationResponse extends BaseWorkOSResource
{
    public const RESOURCE_ATTRIBUTES = [
        "user",
        "organizationId",
        "impersonator",
        "accessToken",
        "refreshToken",
        "oauthTokens",
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "organization_id" => "organizationId",
        "access_token" => "accessToken",
        "refresh_token" => "refreshToken",
        "oauth_tokens" => "oauthTokens",
    ];

    public static function constructFromResponse($response)
    {
        $instance = parent::constructFromResponse($response);

        $instance->values["user"] = User::constructFromResponse($response["user"]);

        if (isset($response["impersonator"])) {
            $instance->values["impersonator"] = Impersonator::constructFromResponse(
                $response["impersonator"]
            );
        }

        if (isset($response["oauth_tokens"])) {
            $instance->values["oauthTokens"] = OAuthTokens::constructFromResponse($response["oauth_tokens"]);
        }

        return $instance;
    }
}
