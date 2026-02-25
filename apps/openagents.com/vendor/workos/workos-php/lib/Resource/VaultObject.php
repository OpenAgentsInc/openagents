<?php

namespace WorkOS\Resource;

/**
 * Class VaultObject.
 */
class VaultObject extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "vault_object";

    public const RESOURCE_ATTRIBUTES = [
        "id",
        "name",
        "updatedAt",
        "value",
        "metadata"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "id" => "id",
        "name" => "name",
        "updated_at" => "updatedAt",
        "value" => "value",
        "metadata" => "metadata"
    ];
}
