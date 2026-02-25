<?php

namespace WorkOS\Resource;

/**
 * Class User.
 */
class User extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "user";

    public const RESOURCE_ATTRIBUTES = [
        "object",
        "id",
        "email",
        "firstName",
        "lastName",
        "emailVerified",
        "profilePictureUrl",
        "lastSignInAt",
        "createdAt",
        "updatedAt",
        "externalId",
        "metadata"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "object" => "object",
        "id" => "id",
        "email" => "email",
        "first_name" => "firstName",
        "last_name" => "lastName",
        "email_verified" => "emailVerified",
        "profile_picture_url" => "profilePictureUrl",
        "last_sign_in_at" => "lastSignInAt",
        "created_at" => "createdAt",
        "updated_at" => "updatedAt",
        "external_id" => "externalId",
        "metadata" => "metadata"
    ];
}
