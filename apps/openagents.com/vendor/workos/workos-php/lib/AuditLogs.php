<?php

namespace WorkOS;

/**
 * Class AuditLogs
 *
 * This class facilitates the use of WorkOS Audit Logs.
 */
class AuditLogs
{
    /**
     * Creates an audit log event for an organization.
     *
     * @param string $organizationId The unique identifier for the organization.
     * @param array  $event          An associative array with the following keys:
     *   - **action** (string, *required*): Specific activity performed by the actor.
     *   - **occurred_at** (string, *required*): ISO-8601 datetime when the event occurred.
     *   - **actor** (array, *required*): Associative array describing the actor.
     *     - **id** (string, *required*): Unique identifier for the actor.
     *     - **name** (string, *optional*): Name of the actor.
     *     - **type** (string, *required*): Type or role of the actor.
     *     - **metadata** (array, *optional*): Arbitrary key-value data.
     *   - **targets** (array, *required*): Array of associative arrays for each target.
     *     Each target includes:
     *     - **id** (string, *required*): Unique identifier for the target.
     *     - **name** (string, *optional*): Name of the target.
     *     - **type** (string, *required*): Type or category of the target.
     *     - **metadata** (array, *optional*): Arbitrary key-value data.
     *   - **context** (array, *required*): Associative array providing additional context.
     *     - **location** (string, *required*): Location associated with the event.
     *     - **user_agent** (string, *optional*): User agent string if applicable.
     *   - **version** (int, *optional*): Event version. Required if the version is not 1.
     *   - **metadata** (array, *optional*): Additional arbitrary key-value data for the event.
     *
     * @param string $idempotencyKey A unique key ensuring idempotency of events for 24 hours.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\AuditLogCreateEventStatus
     */
    public function createEvent($organizationId, $event, ?string $idempotencyKey = null)
    {
        $eventsPath = "audit_logs/events";

        $params = [
            "organization_id" => $organizationId,
            "event" => $event
        ];

        $headers = [
            "idempotency_key" => $idempotencyKey
        ];

        $response = Client::request(Client::METHOD_POST, $eventsPath, $headers, $params, true);

        return Resource\AuditLogCreateEventStatus::constructFromResponse($response);
    }

    /**
     * @param array $auditLogExportOptions Associative array containing the keys detailed below
     * @var null|string $organizationId Description of the record.
     * @var null|string $rangeStart ISO-8601 Timestamp of the start of Export's the date range.
     * @var null|string $rangeEnd ISO-8601 Timestamp  of the end of Export's the date range.
     * @var null|array $actions Actions that Audit Log Events will be filtered by.
     * @var null|array $actors Actor names that Audit Log Events will be filtered by. @deprecated 3.3.0 Use $actorNames instead. This method will be removed in a future major version.
     * @var null|array $targets Target types that Audit Log Events will be filtered by.
     * @var null|array $actorNames Actor names that Audit Log Events will be filtered by.
     * @var null|array $actorIds Actor IDs that Audit Log Events will be filtered by.
     *
     * @throws Exception\WorkOSException
     *
     * @return Resource\AuditLogExport
     */

    public function createExport($organizationId, $rangeStart, $rangeEnd, ?array $actions = null, ?array $actors = null, ?array $targets = null, ?array $actorNames = null, ?array $actorIds = null)
    {
        $createExportPath = "audit_logs/exports";

        $params = [
            "organization_id" => $organizationId,
            "range_end" => $rangeEnd,
            "range_start" => $rangeStart
        ];

        if (!is_null($actions)) {
            $params["actions"] = $actions;
        };

        if (!is_null($actors)) {
            $msg = "'actors' is deprecated. Please use 'actorNames' instead'";

            error_log($msg);

            $params["actors"] = $actors;
        };

        if (!is_null($actorNames)) {
            $params["actor_names"] = $actorNames;
        };

        if (!is_null($actorIds)) {
            $params["actor_ids"] = $actorIds;
        };

        if (!is_null($targets)) {
            $params["targets"] = $targets;
        };

        $response = Client::request(Client::METHOD_POST, $createExportPath, null, $params, true);
        return Resource\AuditLogExport::constructFromResponse($response);
    }

    /**
     * @param string $auditLogExportId Unique identifier of the Audit Log Export
     *
     * @throws Exception\WorkOSException
     * @throws \InvalidArgumentException
     *
     * @return Resource\AuditLogExport
     */
    public function getExport($auditLogExportId)
    {
        // Validate export ID parameter to prevent path traversal
        if (!is_string($auditLogExportId) || !preg_match('/^[a-zA-Z0-9._-]+$/', $auditLogExportId)) {
            throw new \InvalidArgumentException('Invalid export ID format. Export ID must be a string containing only alphanumeric characters, dots, underscores, and hyphens.');
        }

        $getExportPath = "audit_logs/exports/{$auditLogExportId}";

        $response = Client::request(Client::METHOD_GET, $getExportPath, null, null, true);

        return Resource\AuditLogExport::constructFromResponse($response);
    }

    /**
     * Create an audit log action schema.
     *
     * @param string $action The action name for the schema
     * @param array $schema Array containing the schema definition
     *
     * @throws Exception\WorkOSException
     * @throws \InvalidArgumentException
     *
     * @return array The created schema response
     */
    public function createSchema($action, $schema)
    {
        // Validate action parameter to prevent path traversal
        if (!is_string($action) || !preg_match('/^[a-zA-Z0-9._-]+$/', $action)) {
            throw new \InvalidArgumentException('Invalid action format. Action must be a string containing only alphanumeric characters, dots, underscores, and hyphens.');
        }

        $schemaPath = "audit_logs/actions/{$action}/schemas";

        $response = Client::request(Client::METHOD_POST, $schemaPath, null, $schema, true);

        return $response;
    }

    /**
     * Check if an audit log action schema exists.
     *
     * @param string $action The action name to check
     *
     * @throws Exception\WorkOSException
     * @throws \InvalidArgumentException
     *
     * @return bool True if schema exists, false if not found
     */
    public function schemaExists($action)
    {
        // Validate action parameter to prevent path traversal
        if (!is_string($action) || !preg_match('/^[a-zA-Z0-9._-]+$/', $action)) {
            throw new \InvalidArgumentException('Invalid action format. Action must be a string containing only alphanumeric characters, dots, underscores, and hyphens.');
        }

        $schemaPath = "audit_logs/actions/{$action}/schemas";

        try {
            Client::request(Client::METHOD_GET, $schemaPath, null, null, true);
            return true;
        } catch (Exception\NotFoundException $e) {
            return false;
        }
    }

    /**
     * List all registered audit log actions.
     *
     * @param int $limit Maximum number of actions to return (default: 100)
     * @param null|string $before Action ID to look before
     * @param null|string $after Action ID to look after
     * @param null|string $order The order in which to paginate records ("asc" or "desc")
     *
     * @throws Exception\WorkOSException
     *
     * @return array Array of registered actions
     */
    public function listActions($limit = 100, $before = null, $after = null, $order = null)
    {
        $actionsPath = "audit_logs/actions";

        $params = [
            "limit" => $limit
        ];

        if ($before !== null) {
            $params["before"] = $before;
        }

        if ($after !== null) {
            $params["after"] = $after;
        }

        if ($order !== null) {
            $params["order"] = $order;
        }

        $response = Client::request(Client::METHOD_GET, $actionsPath, null, $params, true);

        return $response;
    }
}
