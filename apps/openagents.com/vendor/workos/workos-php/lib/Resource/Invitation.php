<?php

namespace WorkOS\Resource;

/**
 * Class Invitation.
 */

class Invitation extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "invitation";

    public const RESOURCE_ATTRIBUTES = [
        "object",
        "id",
        "email",
        "state",
        "acceptedAt",
        "revokedAt",
        "expiresAt",
        "token",
        "acceptInvitationUrl",
        "organizationId",
        "inviterUserId",
        "createdAt",
        "updatedAt"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "object" => "object",
        "id" => "id",
        "email" => "email",
        "state" => "state",
        "accepted_at" => "acceptedAt",
        "revoked_at" => "revokedAt",
        "expires_at" => "expiresAt",
        "token" => "token",
        "accept_invitation_url" => "acceptInvitationUrl",
        "organization_id" => "organizationId",
        "inviter_user_id" => "inviterUserId",
        "created_at" => "createdAt",
        "updated_at" => "updatedAt"
    ];
}
