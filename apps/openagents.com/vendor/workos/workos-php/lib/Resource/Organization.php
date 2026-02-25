<?php

namespace WorkOS\Resource;

/**
 * Class Organization.
 */

class Organization extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "organization";

    public const RESOURCE_ATTRIBUTES = [
        "id",
        "name",
        "allowProfilesOutsideOrganization",
        "domains",
        "externalId",
        "metadata"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "id" => "id",
        "name" => "name",
        "allow_profiles_outside_organization" => "allowProfilesOutsideOrganization",
        "domains" => "domains",
        "external_id" => "externalId",
        "metadata" => "metadata"
    ];
}
