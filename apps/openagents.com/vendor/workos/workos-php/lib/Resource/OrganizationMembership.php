<?php

namespace WorkOS\Resource;

use WorkOS\Resource\RoleResponse;

/**
 * Class OrganizationMembership.
 *
 * @property 'organization_membership' $object
 * @property string $id
 * @property string $userId
 * @property string $organizationId
 * @property RoleResponse $role
 * @property array<RoleResponse> $roles
 * @property 'active'|'inactive'|'pending' $status
 * @property string $createdAt
 * @property string $updatedAt
 */
class OrganizationMembership extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "organization_membership";

    public const RESOURCE_ATTRIBUTES = [
        "object",
        "id",
        "userId",
        "organizationId",
        "role",
        "roles",
        "status",
        "createdAt",
        "updatedAt"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "object" => "object",
        "id" => "id",
        "user_id" => "userId",
        "organization_id" => "organizationId",
        "role" => "role",
        "roles" => "roles",
        "status" => "status",
        "created_at" => "createdAt",
        "updated_at" => "updatedAt"
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
