<?php

namespace WorkOS\Resource;

/**
 * Class UserAuthenticationFactorTotp.
 */
class UserAuthenticationFactorTotp extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "authentication_factor";

    public const RESOURCE_ATTRIBUTES = [
        "object",
        "id",
        "userId",
        "createdAt",
        "updatedAt",
        "type",
        "totp"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "object" => "object",
        "id" => "id",
        "user_id" => "userId",
        "created_at" => "createdAt",
        "updated_at" => "updatedAt",
        "type" => "type",
        "totp" => "totp"
    ];
}
