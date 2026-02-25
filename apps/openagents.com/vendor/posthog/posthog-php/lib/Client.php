<?php

namespace PostHog;

use Exception;
use PostHog\Consumer\File;
use PostHog\Consumer\ForkCurl;
use PostHog\Consumer\LibCurl;
use PostHog\Consumer\Socket;

const SIZE_LIMIT = 50_000;

class Client
{
    private const CONSUMERS = [
        "socket" => Socket::class,
        "file" => File::class,
        "fork_curl" => ForkCurl::class,
        "lib_curl" => LibCurl::class,
    ];


    /**
     * @var string
     */
    private $apiKey;

    /**
     * @var string
     */
    private $personalAPIKey;

    /**
     * @var integer
     */
    private $featureFlagsRequestTimeout;

    /**
     * Consumer object handles queueing and bundling requests to PostHog.
     *
     * @var Consumer
     */
    protected $consumer;

    /**
     * @var HttpClient
     */
    public $httpClient;

    /**
     * @var array
     */
    public $featureFlags;

    /**
     * @var array
     */
    public $groupTypeMapping;

    /**
     * @var array
     */
    public $cohorts;

    /**
     * @var array
     */
    public $featureFlagsByKey;

    /**
     * @var SizeLimitedHash
     */
    public $distinctIdsFeatureFlagsReported;

    /**
     * @var string|null Cached ETag for feature flag definitions
     */
    private $flagsEtag;

    /**
     * @var bool
     */
    private $debug;

    /**
     * Create a new posthog object with your app's API key
     * key
     *
     * @param string $apiKey
     * @param array $options array of consumer options [optional]
     * @param HttpClient|null $httpClient
     */
    public function __construct(
        string $apiKey,
        array $options = [],
        ?HttpClient $httpClient = null,
        ?string $personalAPIKey = null,
        bool $loadFeatureFlags = true,
    ) {
        $this->apiKey = $apiKey;
        $this->personalAPIKey = $personalAPIKey;
        $this->debug = $options["debug"] ?? false;
        $Consumer = self::CONSUMERS[$options["consumer"] ?? "lib_curl"];
        $this->consumer = new $Consumer($apiKey, $options, $httpClient);
        $this->httpClient = $httpClient !== null ? $httpClient : new HttpClient(
            $options['host'] ?? "app.posthog.com",
            $options['ssl'] ?? true,
            (int) ($options['maximum_backoff_duration'] ?? 10000),
            false,
            $options["debug"] ?? false,
            null,
            (int) ($options['timeout'] ?? 10000)
        );
        $this->featureFlagsRequestTimeout = (int) ($options['feature_flag_request_timeout_ms'] ?? 3000);
        $this->featureFlags = [];
        $this->groupTypeMapping = [];
        $this->cohorts = [];
        $this->featureFlagsByKey = [];
        $this->distinctIdsFeatureFlagsReported = new SizeLimitedHash(SIZE_LIMIT);
        $this->flagsEtag = null;

        // Populate featureflags and grouptypemapping if possible
        if (
            count($this->featureFlags) == 0
            && !is_null($this->personalAPIKey)
            && $loadFeatureFlags
        ) {
            $this->loadFlags();
        }
    }

    public function __destruct()
    {
        $this->consumer->__destruct();
    }

    /**
     * Captures a user action
     *
     * @param array $message
     * @return bool whether the capture call succeeded
     */
    public function capture(array $message)
    {
        $message = $this->message($message);
        $message["type"] = "capture";

        if (array_key_exists('$groups', $message)) {
            $message["properties"]['$groups'] = $message['$groups'];
        }

        $extraProperties = [];
        $flags = [];
        if (array_key_exists("send_feature_flags", $message) && $message["send_feature_flags"]) {
            $flags = $this->fetchFeatureVariants($message["distinct_id"], $message["groups"]);
        } elseif (count($this->featureFlags) != 0) {
            # Local evaluation is enabled, flags are loaded, so try and get all flags we can without going to the server
            $flags = $this->getAllFlags($message["distinct_id"], $message["groups"], [], [], true);
        }

        // Add all feature variants to event
        foreach ($flags as $flagKey => $flagValue) {
            $extraProperties[sprintf('$feature/%s', $flagKey)] = $flagValue;
        }

        // Add all feature flag keys that aren't false to $active_feature_flags
        // decide v2 does this automatically, but we need it for when we upgrade to v3
        $extraProperties['$active_feature_flags'] = array_keys(array_filter($flags, function ($flagValue) {
            return $flagValue !== false;
        }));

        $message["properties"] = array_merge($extraProperties, $message["properties"]);

        return $this->consumer->capture($message);
    }

    /**
     * Tags properties about the user.
     *
     * @param array $message
     * @return bool whether the identify call succeeded
     */
    public function identify(array $message)
    {
        if (isset($message['properties'])) {
            $message['$set'] = $message['properties'];
        }

        $message = $this->message($message);
        $message["type"] = "identify";
        $message["event"] = '$identify';

        return $this->consumer->identify($message);
    }

    /**
     * decide if the feature flag is enabled for this distinct id.
     *
     * @param string $key
     * @param string $distinctId
     * @param array $groups
     * @param array $personProperties
     * @param array $groupProperties
     * @return bool
     * @throws Exception
     */
    public function isFeatureEnabled(
        string $key,
        string $distinctId,
        array $groups = array(),
        array $personProperties = array(),
        array $groupProperties = array(),
        bool $onlyEvaluateLocally = false,
        bool $sendFeatureFlagEvents = true
    ): null | bool {
        $result = $this->getFeatureFlag(
            $key,
            $distinctId,
            $groups,
            $personProperties,
            $groupProperties,
            $onlyEvaluateLocally,
            $sendFeatureFlagEvents
        );

        if (is_null($result)) {
            return $result;
        } else {
            return boolval($result);
        }
    }

    /**
     * get the feature flag value for this distinct id.
     *
     * @param string $key
     * @param string $distinctId
     * @param array $groups
     * @param array $personProperties
     * @param array $groupProperties
     * @return bool | string
     * @throws Exception
     */
    public function getFeatureFlag(
        string $key,
        string $distinctId,
        array $groups = array(),
        array $personProperties = array(),
        array $groupProperties = array(),
        bool $onlyEvaluateLocally = false,
        bool $sendFeatureFlagEvents = true
    ): null | bool | string {
        [$personProperties, $groupProperties] = $this->addLocalPersonAndGroupProperties(
            $distinctId,
            $groups,
            $personProperties,
            $groupProperties
        );
        $result = null;

        foreach ($this->featureFlags as $flag) {
            if ($flag["key"] == $key) {
                try {
                    $result = $this->computeFlagLocally(
                        $flag,
                        $distinctId,
                        $groups,
                        $personProperties,
                        $groupProperties
                    );
                } catch (RequiresServerEvaluationException $e) {
                    $result = null;
                } catch (InconclusiveMatchException $e) {
                    $result = null;
                } catch (Exception $e) {
                    $result = null;
                    error_log("[PostHog][Client] Error while computing variant:" . $e->getMessage());
                }
            }
        }

        $flagWasEvaluatedLocally = !is_null($result);
        $requestId = null;
        $evaluatedAt = null;
        $flagDetail = null;

        if (!$flagWasEvaluatedLocally && !$onlyEvaluateLocally) {
            try {
                $response = $this->fetchFlagsResponse($distinctId, $groups, $personProperties, $groupProperties);
                $requestId = isset($response['requestId']) ? $response['requestId'] : null;
                $evaluatedAt = isset($response['evaluatedAt']) ? $response['evaluatedAt'] : null;
                $flagDetail = isset($response['flags'][$key]) ? $response['flags'][$key] : null;
                $featureFlags = $response['featureFlags'] ?? [];
                if (array_key_exists($key, $featureFlags)) {
                    $result = $featureFlags[$key];
                } else {
                    $result = null;
                }
            } catch (Exception $e) {
                error_log("[PostHog][Client] Unable to get feature variants:" . $e->getMessage());
                $result = null;
            }
        }

        if ($sendFeatureFlagEvents && !$this->distinctIdsFeatureFlagsReported->contains($key, $distinctId)) {
            $properties = [
                '$feature_flag' => $key,
                '$feature_flag_response' => $result,
            ];

            if (!is_null($requestId)) {
                $properties['$feature_flag_request_id'] = $requestId;
            }

            if (!is_null($evaluatedAt)) {
                $properties['$feature_flag_evaluated_at'] = $evaluatedAt;
            }

            if (!is_null($flagDetail)) {
                $properties['$feature_flag_id'] = $flagDetail['metadata']['id'];
                $properties['$feature_flag_version'] = $flagDetail['metadata']['version'];
                $properties['$feature_flag_reason'] = $flagDetail['reason']['description'];
            }

            $this->capture([
                "properties" => $properties,
                "distinct_id" => $distinctId,
                "event" => '$feature_flag_called',
                '$groups' => $groups
            ]);
            $this->distinctIdsFeatureFlagsReported->add($key, $distinctId);
        }

        if (!is_null($result)) {
            return $result;
        }
        return null;
    }

    /**
     * @param string $key
     * @param string $distinctId
     * @param array $groups
     * @param array $personProperties
     * @param array $groupProperties
     * @return mixed
     */
    public function getFeatureFlagPayload(
        string $key,
        string $distinctId,
        array $groups = array(),
        array $personProperties = array(),
        array $groupProperties = array(),
    ): mixed {
        $results = json_decode(
            $this->flags($distinctId, $groups, $personProperties, $groupProperties),
            true
        );

        if (isset($results['featureFlags'][$key]) === false || $results['featureFlags'][$key] !== true) {
            return null;
        }

        $payload = $results['featureFlagPayloads'][$key] ?? null;

        if ($payload === null) {
            return null;
        }

        # feature flag payloads are always JSON encoded strings.
        return json_decode($payload, true);
    }

    /**
     * get the feature flag value for this distinct id.
     *
     * @param string $distinctId
     * @param array $groups
     * @param array $personProperties
     * @param array $groupProperties
     * @return array
     * @throws Exception
     */
    public function getAllFlags(
        string $distinctId,
        array $groups = array(),
        array $personProperties = array(),
        array $groupProperties = array(),
        bool $onlyEvaluateLocally = false
    ): array {
        [$personProperties, $groupProperties] = $this->addLocalPersonAndGroupProperties(
            $distinctId,
            $groups,
            $personProperties,
            $groupProperties
        );
        $response = [];
        $fallbackToFlags = false;

        if (count($this->featureFlags) > 0) {
            foreach ($this->featureFlags as $flag) {
                try {
                    $response[$flag['key']] = $this->computeFlagLocally(
                        $flag,
                        $distinctId,
                        $groups,
                        $personProperties,
                        $groupProperties
                    );
                } catch (RequiresServerEvaluationException $e) {
                    $fallbackToFlags = true;
                } catch (InconclusiveMatchException $e) {
                    $fallbackToFlags = true;
                } catch (Exception $e) {
                    $fallbackToFlags = true;
                    error_log("[PostHog][Client] Error while computing variant:" . $e->getMessage());
                }
            }
        } else {
            $fallbackToFlags = true;
        }

        if ($fallbackToFlags && !$onlyEvaluateLocally) {
            try {
                $featureFlags = $this->fetchFeatureVariants($distinctId, $groups, $personProperties, $groupProperties);
                $response = array_merge($response, $featureFlags);
            } catch (Exception $e) {
                error_log("[PostHog][Client] Unable to get feature variants:" . $e->getMessage());
            }
        }

        return $response;
    }

    private function computeFlagLocally(
        array $featureFlag,
        string $distinctId,
        array $groups = array(),
        array $personProperties = array(),
        array $groupProperties = array()
    ): bool | string {
        // Create evaluation cache for flag dependencies
        $evaluationCache = [];

        if ($featureFlag["ensure_experience_continuity"] ?? false) {
            throw new InconclusiveMatchException("Flag has experience continuity enabled");
        }

        if (!$featureFlag["active"]) {
            return false;
        }

        $flagFilters = $featureFlag["filters"] ?? [];
        $aggregationGroupTypeIndex = $flagFilters["aggregation_group_type_index"] ?? null;

        if (!is_null($aggregationGroupTypeIndex)) {
            $groupName = $this->groupTypeMapping[strval($aggregationGroupTypeIndex)] ?? null;

            if (is_null($groupName)) {
                throw new InconclusiveMatchException("Flag has unknown group type index");
            }

            if (!array_key_exists($groupName, $groups)) {
                return false;
            }

            $focusedGroupProperties = $groupProperties[$groupName];
            return FeatureFlag::matchFeatureFlagProperties(
                $featureFlag,
                $groups[$groupName],
                $focusedGroupProperties,
                $this->cohorts,
                $this->featureFlagsByKey,
                $evaluationCache
            );
        } else {
            return FeatureFlag::matchFeatureFlagProperties(
                $featureFlag,
                $distinctId,
                $personProperties,
                $this->cohorts,
                $this->featureFlagsByKey,
                $evaluationCache
            );
        }
    }


    /**
     * @param string $distinctId
     * @param array $groups
     * @return array of feature flags
     * @throws Exception
     */
    public function fetchFeatureVariants(
        string $distinctId,
        array $groups = [],
        array $personProperties = [],
        array $groupProperties = []
    ): array {
        $response = $this->fetchFlagsResponse($distinctId, $groups, $personProperties, $groupProperties);
        return $response['featureFlags'] ?? [];
    }

    /**
     * @param string $distinctId
     * @param array $groups
     * @return array of feature flags
     * @throws Exception
     */
    private function fetchFlagsResponse(
        string $distinctId,
        array $groups = [],
        array $personProperties = [],
        array $groupProperties = []
    ): ?array {
        return json_decode(
            $this->flags($distinctId, $groups, $personProperties, $groupProperties),
            true
        );
    }

    /**
     * @throws Exception
     */

    public function loadFlags()
    {
        $response = $this->localFlags();

        // Handle 304 Not Modified - flags haven't changed, skip processing.
        // On 304, we preserve the existing ETag unless the server sends a new one.
        // This handles edge cases like server restarts where the server may send
        // a refreshed ETag even though the content hasn't changed.
        if ($response->isNotModified()) {
            if ($response->getEtag()) {
                $this->flagsEtag = $response->getEtag();
            }
            if ($this->debug) {
                error_log("[PostHog][Client] Flags not modified (304), using cached data");
            }
            return;
        }

        $payload = json_decode($response->getResponse(), true);

        if ($payload && array_key_exists("detail", $payload)) {
            throw new Exception($payload["detail"]);
        }

        // On 200 responses, always update ETag (even if null) since we're replacing
        // the cached flag data. A null ETag means the server doesn't support caching.
        $this->flagsEtag = $response->getEtag();

        $this->featureFlags = $payload['flags'] ?? [];
        $this->groupTypeMapping = $payload['group_type_mapping'] ?? [];
        $this->cohorts = $payload['cohorts'] ?? [];

        // Build flags by key dictionary for dependency resolution
        $this->featureFlagsByKey = [];
        foreach ($this->featureFlags as $flag) {
            $this->featureFlagsByKey[$flag['key']] = $flag;
        }
    }


    public function localFlags(): HttpResponse
    {
        $headers = [
            // Send user agent in the form of {library_name}/{library_version} as per RFC 7231.
            "User-Agent: posthog-php/" . PostHog::VERSION,
            "Authorization: Bearer " . $this->personalAPIKey
        ];

        // Add If-None-Match header if we have a cached ETag
        if ($this->flagsEtag !== null) {
            $headers[] = "If-None-Match: " . $this->flagsEtag;
        }

        return $this->httpClient->sendRequest(
            '/api/feature_flag/local_evaluation?send_cohorts&token=' . $this->apiKey,
            null,
            $headers,
            [
                'includeEtag' => true
            ]
        );
    }

    /**
     * Get the current cached ETag for feature flag definitions
     *
     * @return string|null
     */
    public function getFlagsEtag(): ?string
    {
        return $this->flagsEtag;
    }

    private function normalizeFeatureFlags(string $response): string
    {
        $decoded = json_decode($response, true);
        if (isset($decoded['flags']) && !empty($decoded['flags'])) {
            // This is a v4 response, we need to transform it to a v3 response for backwards compatibility
            $transformedFlags = [];
            $transformedPayloads = [];
            foreach ($decoded['flags'] as $key => $flag) {
                if ($flag['variant'] !== null) {
                    $transformedFlags[$key] = $flag['variant'];
                } else {
                    $transformedFlags[$key] = $flag['enabled'] ?? false;
                }
                if (isset($flag['metadata']['payload'])) {
                    $transformedPayloads[$key] = $flag['metadata']['payload'];
                }
            }
            $decoded['featureFlags'] = $transformedFlags;
            $decoded['featureFlagPayloads'] = $transformedPayloads;
            return json_encode($decoded);
        }

        return $response;
    }

    public function flags(
        string $distinctId,
        array $groups = array(),
        array $personProperties = [],
        array $groupProperties = []
    ) {
        $payload = array(
            'api_key' => $this->apiKey,
            'distinct_id' => $distinctId,
        );

        if (!empty($groups)) {
            $payload["groups"] = $groups;
        }

        if (!empty($personProperties)) {
            $payload["person_properties"] = $personProperties;
        }

        if (!empty($groupProperties)) {
            $payload["group_properties"] = $groupProperties;
        }

        $response = $this->httpClient->sendRequest(
            '/flags/?v=2',
            json_encode($payload),
            [
                // Send user agent in the form of {library_name}/{library_version} as per RFC 7231.
                "User-Agent: posthog-php/" . PostHog::VERSION,
            ],
            [
                "shouldRetry" => false,
                "timeout" => $this->featureFlagsRequestTimeout
            ]
        )->getResponse();

        return $this->normalizeFeatureFlags($response);
    }

    /**
     * Aliases from one user id to another
     *
     * @param array $message
     * @return boolean whether the alias call succeeded
     */
    public function alias(array $message)
    {
        $message = $this->message($message);
        $message["type"] = "alias";
        $message["event"] = '$create_alias';

        $message['properties']['distinct_id'] = $message['distinct_id'];
        $message['properties']['alias'] = $message['alias'];

        $message['distinct_id'] = null;
        unset($message['alias']);

        return $this->consumer->alias($message);
    }

    /**
     * Queue a raw (prepared) message
     *
     * @param array $message
     * @return mixed whether the identify call succeeded
     */
    public function raw(array $message)
    {
        return $this->consumer->enqueue($message);
    }

    /**
     * Flush any async consumers
     * @return boolean true if flushed successfully
     */
    public function flush()
    {
        if (method_exists($this->consumer, 'flush')) {
            return $this->consumer->flush();
        }

        return true;
    }

    /**
     * Formats a timestamp by making sure it is set
     * and converting it to iso8601.
     *
     * The timestamp can be time in seconds `time()` or `microseconds(true)`.
     * any other input is considered an error and the method will return a new date.
     *
     * Note: php's date() "u" format (for microseconds) has a bug in it
     * it always shows `.000` for microseconds since `date()` only accepts
     * ints, so we have to construct the date ourselves if microtime is passed.
     *
     * @param $ts
     * @return false|string
     */
    private function formatTime($ts)
    {
        // time()
        if (null == $ts || !$ts) {
            $ts = time();
        }
        if (false !== filter_var($ts, FILTER_VALIDATE_INT)) {
            return date("c", (int)$ts);
        }

        // anything else try to strtotime the date.
        if (false === filter_var($ts, FILTER_VALIDATE_FLOAT)) {
            if (is_string($ts)) {
                return date("c", strtotime($ts));
            }

            return date("c");
        }

        // fix for floatval casting in send.php
        $parts = explode(".", (string)$ts);
        if (!isset($parts[1])) {
            return date("c", (int)$parts[0]);
        }

        // microtime(true)
        $sec = (int)$parts[0];
        $usec = (int)$parts[1];
        $fmt = sprintf("Y-m-d\\TH:i:s%sP", $usec);

        return date($fmt, (int)$sec);
    }

    /**
     * Add common fields to the given `message`
     *
     * @param array $msg
     * @return array
     */
    private function message($msg)
    {
        if (!isset($msg["properties"])) {
            $msg["properties"] = array();
        }

        $msg["library"] = 'posthog-php';
        $msg["library_version"] = PostHog::VERSION;
        $msg["library_consumer"] = $this->consumer->getConsumer();

        $msg["properties"]['$lib'] = 'posthog-php';
        $msg["properties"]['$lib_version'] = PostHog::VERSION;
        $msg["properties"]['$lib_consumer'] = $this->consumer->getConsumer();

        if (isset($msg["distinctId"])) {
            $msg["distinct_id"] = $msg["distinctId"];
            unset($msg["distinctId"]);
        }

        if (isset($msg["sendFeatureFlags"])) {
            $msg["send_feature_flags"] = $msg["sendFeatureFlags"];
            unset($msg["sendFeatureFlags"]);
        }

        if (!isset($msg["groups"])) {
            $msg["groups"] = [];
        }

        if (!isset($msg["timestamp"])) {
            $msg["timestamp"] = null;
        }
        $msg["timestamp"] = $this->formatTime($msg["timestamp"]);

        return $msg;
    }

    private function addLocalPersonAndGroupProperties(
        string $distinctId,
        array $groups,
        array $personProperties,
        array $groupProperties
    ): array {
        $allPersonProperties = array_merge(
            ["distinct_id" => $distinctId],
            $personProperties
        );

        $allGroupProperties = [];
        if (count($groups) > 0) {
            foreach ($groups as $groupName => $groupValue) {
                $allGroupProperties[$groupName] = array_merge(
                    ["\$group_key" => $groupValue],
                    $groupProperties[$groupName] ?? []
                );
            }
        }

        return [$allPersonProperties, $allGroupProperties];
    }
}
