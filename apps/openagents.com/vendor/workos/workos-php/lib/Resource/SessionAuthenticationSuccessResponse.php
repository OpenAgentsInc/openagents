<?php

namespace WorkOS\Resource;

/**
 * Class SessionAuthenticationSuccessResponse.
 *
 * Represents a successful session authentication.
 *
 * @property bool $authenticated
 * @property string $accessToken
 * @property string $refreshToken
 * @property string $sessionId
 * @property User $user
 * @property string|null $organizationId
 * @property RoleResponse|null $role
 * @property array|null $roles
 * @property array|null $permissions
 * @property array|null $entitlements
 * @property array|null $featureFlags
 * @property Impersonator|null $impersonator
 * @property string $authenticationMethod
 */
class SessionAuthenticationSuccessResponse extends BaseWorkOSResource
{
    public const RESOURCE_ATTRIBUTES = [
        "authenticated",
        "accessToken",
        "refreshToken",
        "sessionId",
        "user",
        "organizationId",
        "role",
        "roles",
        "permissions",
        "entitlements",
        "featureFlags",
        "impersonator",
        "authenticationMethod"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "authenticated" => "authenticated",
        "access_token" => "accessToken",
        "refresh_token" => "refreshToken",
        "session_id" => "sessionId",
        "user" => "user",
        "organization_id" => "organizationId",
        "role" => "role",
        "roles" => "roles",
        "permissions" => "permissions",
        "entitlements" => "entitlements",
        "feature_flags" => "featureFlags",
        "impersonator" => "impersonator",
        "authentication_method" => "authenticationMethod"
    ];

    public static function constructFromResponse($response)
    {
        $instance = parent::constructFromResponse($response);

        // Always set authenticated to true for success responses
        $instance->values["authenticated"] = true;

        // Construct User resource from user data
        if (isset($response["user"])) {
            $instance->values["user"] = User::constructFromResponse($response["user"]);
        }

        // Construct Role if present
        if (isset($response["role"])) {
            $instance->values["role"] = new RoleResponse($response["role"]["slug"]);
        }

        // Construct Roles array if present
        if (isset($response["roles"])) {
            $roles = [];
            foreach ($response["roles"] as $role) {
                $roles[] = new RoleResponse($role["slug"]);
            }
            $instance->values["roles"] = $roles;
        }

        // Construct FeatureFlags array if present
        if (isset($response["feature_flags"])) {
            $featureFlags = [];
            foreach ($response["feature_flags"] as $flag) {
                $featureFlags[] = FeatureFlag::constructFromResponse($flag);
            }
            $instance->values["featureFlags"] = $featureFlags;
        }

        // Construct Impersonator if present
        if (isset($response["impersonator"])) {
            $instance->values["impersonator"] = Impersonator::constructFromResponse(
                $response["impersonator"]
            );
        }

        return $instance;
    }
}
