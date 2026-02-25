<?php

namespace WorkOS\Resource;

/**
 * Class EmailVerification.
 */

class EmailVerification extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "email_verification";

    public const RESOURCE_ATTRIBUTES = [
        "object",
        "id",
        "userId",
        "email",
        "expiresAt",
        "code",
        "createdAt",
        "updatedAt"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "object" => "object",
        "id" => "id",
        "user_id" => "userId",
        "email" => "email",
        "expires_at" => "expiresAt",
        "code" => "code",
        "created_at" => "createdAt",
        "updated_at" => "updatedAt"
    ];
}
