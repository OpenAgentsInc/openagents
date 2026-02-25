<?php

namespace WorkOS\Resource;

/**
 * class PasswordlessSession.
 */
class PasswordlessSession extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "passwordless_session";

    public const RESOURCE_ATTRIBUTES = [
        "id",
        "email",
        "expiresAt",
        "link",
        "object"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "id" => "id",
        "email" => "email",
        "expires_at" => "expiresAt",
        "link" => "link",
        "object" => "object"
    ];
}
