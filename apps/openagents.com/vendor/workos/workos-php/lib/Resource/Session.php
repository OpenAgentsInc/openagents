<?php

namespace WorkOS\Resource;

/**
 * Class Session.
 *
 * @property string $id
 * @property string $userId
 * @property string|null $ipAddress
 * @property string|null $userAgent
 * @property string|null $organizationId
 * @property string $authenticationMethod
 * @property string $status
 * @property string $expiresAt
 * @property string|null $endedAt
 * @property string $createdAt
 * @property string $updatedAt
 * @property string $object
 */
class Session extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "session";

    public const RESOURCE_ATTRIBUTES = [
        "id",
        "userId",
        "ipAddress",
        "userAgent",
        "organizationId",
        "authenticationMethod",
        "status",
        "expiresAt",
        "endedAt",
        "createdAt",
        "updatedAt",
        "object"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "id" => "id",
        "user_id" => "userId",
        "ip_address" => "ipAddress",
        "user_agent" => "userAgent",
        "organization_id" => "organizationId",
        "authentication_method" => "authenticationMethod",
        "status" => "status",
        "expires_at" => "expiresAt",
        "ended_at" => "endedAt",
        "created_at" => "createdAt",
        "updated_at" => "updatedAt",
        "object" => "object"
    ];
}
