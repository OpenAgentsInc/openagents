<?php

namespace WorkOS\Resource;

/**
 * Class PasswordReset.
 */

class PasswordReset extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "password_reset";

    public const RESOURCE_ATTRIBUTES = [
        "object",
        "id",
        "userId",
        "email",
        "passwordResetToken",
        "passwordResetUrl",
        "expiresAt",
        "createdAt",
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "object" => "object",
        "id" => "id",
        "user_id" => "userId",
        "email" => "email",
        "password_reset_token" => "passwordResetToken",
        "password_reset_url" => "passwordResetUrl",
        "expires_at" => "expiresAt",
        "created_at" => "createdAt",
    ];
}
