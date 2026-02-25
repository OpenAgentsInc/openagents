<?php

namespace WorkOS\Resource;

use WorkOS\Resource\RoleResponse;

/**
 * Class Profile.
 *
 * @property string $id
 * @property string $email
 * @property string $firstName
 * @property string $lastName
 * @property string $organizationId
 * @property string $connectionId
 * @property string $connectionType
 * @property string $idpId
 * @property RoleResponse|null $role
 * @property array<RoleResponse>|null $roles
 * @property array  $groups
 * @property array  $rawAttributes
 */
class Profile extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "profile";

    public const RESOURCE_ATTRIBUTES = [
        "id",
        "email",
        "firstName",
        "lastName",
        "organizationId",
        "connectionId",
        "connectionType",
        "idpId",
        "role",
        "roles",
        "groups",
        "customAttributes",
        "rawAttributes"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "id" => "id",
        "email" => "email",
        "first_name" => "firstName",
        "last_name" => "lastName",
        "organization_id" => "organizationId",
        "connection_id" => "connectionId",
        "connection_type" => "connectionType",
        "idp_id" => "idpId",
        "role" => "role",
        "roles" => "roles",
        "groups" => "groups",
        "custom_attributes" => "customAttributes",
        "raw_attributes" => "rawAttributes"
    ];

    public static function constructFromResponse($response)
    {
        $instance = parent::constructFromResponse($response);

        if (isset($response["role"])) {
            $instance->values["role"] = new RoleResponse($response["role"]["slug"]);
        }

        if (isset($response["roles"])) {
            $roles = [];
            foreach ($response["roles"] as $role) {
                $roles[] = new RoleResponse($role["slug"]);
            }
            $instance->values["roles"] = $roles;
        }

        return $instance;
    }
}
