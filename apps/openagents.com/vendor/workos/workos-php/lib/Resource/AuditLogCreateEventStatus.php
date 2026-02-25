<?php

namespace WorkOS\Resource;

/**
 * Class AuditLogCreateEventStatus.
 */
class AuditLogCreateEventStatus extends BaseWorkOSResource
{
    public const RESOURCE_TYPE = "audit_log_create_event_status";

    public const RESOURCE_ATTRIBUTES = [
        "success"
    ];

    public const RESPONSE_TO_RESOURCE_KEY = [
        "success" => "success"
    ];
}
