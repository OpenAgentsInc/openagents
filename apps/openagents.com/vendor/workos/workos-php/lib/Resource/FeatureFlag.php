<?php

namespace WorkOS\Resource;

/**
 * Class FeatureFlag.
 *
 * @property string $id
 * @property string $slug
 * @property string $name
 * @property string $description
 * @property string $createdAt
 * @property string $updatedAt
 */

class FeatureFlag extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "feature_flag";

    public const RESOURCE_ATTRIBUTES = [
        "id",
        "slug",
        "name",
        "description",
        "createdAt",
        "updatedAt"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "id" => "id",
        "slug" => "slug",
        "name" => "name",
        "description" => "description",
        "created_at" => "createdAt",
        "updated_at" => "updatedAt"
    ];
}
