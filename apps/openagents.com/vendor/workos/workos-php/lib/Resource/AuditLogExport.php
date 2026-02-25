<?php

namespace WorkOS\Resource;

/**
 * Class AuditLogExport.
 */
class AuditLogExport extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "audit_log_export";

    public const RESOURCE_ATTRIBUTES = [
        "object",
        "id",
        "state",
        "url",
        "createdAt",
        "updatedAt"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "object" => "object",
        "id" => "id",
        "state" => "state",
        "url" => "url",
        "created_at" => "createdAt",
        "updated_at" => "updatedAt"
    ];
}
