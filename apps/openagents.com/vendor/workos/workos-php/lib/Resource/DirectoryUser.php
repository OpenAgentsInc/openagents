<?php

namespace WorkOS\Resource;

use WorkOS\Resource\RoleResponse;

/**
 * Class DirectoryUser.
 *
 * @property RoleResponse|null $role
 * @property array<RoleResponse>|null $roles
 */
class DirectoryUser extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "directory_usr";

    public const RESOURCE_ATTRIBUTES = [
        "id",
        "rawAttributes",
        "customAttributes",
        "firstName",
        "email",
        /**
         * @deprecated 4.22.0 Will be removed in a future major version.
         * Enable the `emails` custom attribute in dashboard and pull from customAttributes instead.
         * See https://workos.com/docs/directory-sync/attributes/custom-attributes/auto-mapped-attributes for details.
         */
        "emails",
        /**
         * @deprecated 4.22.0 Will be removed in a future major version.
         * Enable the `username` custom attribute in dashboard and pull from customAttributes instead.
         * See https://workos.com/docs/directory-sync/attributes/custom-attributes/auto-mapped-attributes for details.
         */
        "username",
        "lastName",
        /**
         * @deprecated 4.22.0 Will be removed in a future major version.
         * Enable the `job_title` custom attribute in dashboard and pull from customAttributes instead.
         * See https://workos.com/docs/directory-sync/attributes/custom-attributes/auto-mapped-attributes for details.
         */
        "jobTitle",
        "state",
        "idpId",
        "role",
        "roles",
        "groups",
        "directoryId",
        "organizationId"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "id" => "id",
        "raw_attributes" => "rawAttributes",
        "custom_attributes" => "customAttributes",
        "first_name" => "firstName",
        "email" => "email",
        "emails" => "emails",
        "username" => "username",
        "last_name" => "lastName",
        "job_title" => "jobTitle",
        "state" => "state",
        "idp_id" => "idpId",
        "role" => "role",
        "roles" => "roles",
        "groups" => "groups",
        "directory_id" => "directoryId",
        "organization_id" => "organizationId"
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

    /**
     * @deprecated 4.22.0 Use `email` property instead.
     *
     * @return string|null The primary email address if found, null otherwise
     */
    public function primaryEmail()
    {
        $response = $this;

        if (count($response->raw["emails"]) == 0) {
            return;
        };

        foreach ($response->raw["emails"] as $value) {
            if ($value["primary"] == true) {
                return $value["value"];
            };
        };
    }
}
