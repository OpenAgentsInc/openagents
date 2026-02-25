<?php

namespace WorkOS\Resource;

/**
 * Class AuthenticationFactorTotp.
 */
class AuthenticationFactorTotp extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "authentication_factor";

    public const RESOURCE_ATTRIBUTES = [
        "object",
        "id",
        "createdAt",
        "updatedAt",
        "type",
        "totp"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "object" => "object",
        "id" => "id",
        "created_at" => "createdAt",
        "updated_at" => "updatedAt",
        "type" => "type",
        "totp" => "totp"
    ];
}
