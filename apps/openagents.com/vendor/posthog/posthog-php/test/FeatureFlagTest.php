<?php

namespace PostHog\Test;

// comment out below to print all logs instead of failing tests
require_once 'test/error_log_mock.php';

use Exception;
use PHPUnit\Framework\TestCase;
use PostHog\Client;
use PostHog\PostHog;
use PostHog\Test\Assets\MockedResponses;
use SlopeIt\ClockMock\ClockMock;


class FeatureFlagTest extends TestCase
{
    const FAKE_API_KEY = "random_key";

    private $http_client;
    private $client;

    public function setUp($flagsEndpointResponse = MockedResponses::FLAGS_RESPONSE, $personalApiKey = "test"): void
    {
        date_default_timezone_set("UTC");
        $this->http_client = new MockedHttpClient("app.posthog.com", flagsEndpointResponse: $flagsEndpointResponse);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            $personalApiKey
        );
        PostHog::init(null, null, $this->client);

        // Reset the errorMessages array before each test
        global $errorMessages;
        $errorMessages = [];
    }

    public function checkEmptyErrorLogs(): void
    {
        global $errorMessages;
        $this->assertTrue(empty($errorMessages), "Error logs are not empty: " . implode("\n", $errorMessages));
    }

    public function decideResponseCases(): array
    {
        return [
            'v3 response' => [MockedResponses::FLAGS_RESPONSE],
            'v4 response' => [MockedResponses::FLAGS_V2_RESPONSE]
        ];
    }

    /**
     * @dataProvider decideResponseCases
     */
    public function testIsFeatureEnabled($response)
    {
        $this->setUp($response);
        $this->assertFalse(PostHog::isFeatureEnabled('having_fun', 'user-id'));
        $this->assertEquals(
            $this->http_client->calls,
            array(
                0 => array(
                    "path" => "/api/feature_flag/local_evaluation?send_cohorts&token=random_key",
                    "payload" => null,
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION, 1 => 'Authorization: Bearer test'),
                    "requestOptions" => array("includeEtag" => true),
                ),
                1 => array(
                    "path" => "/flags/?v=2",
                    "payload" => sprintf('{"api_key":"%s","distinct_id":"user-id","person_properties":{"distinct_id":"user-id"}}', self::FAKE_API_KEY),
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                    "requestOptions" => array("timeout" => 3000, "shouldRetry" => false),
                ),
            )
        );
    }

    public function testIsFeatureEnabledCapturesFeatureFlagCalledEventWithAdditionalMetadata()
    {
        ClockMock::executeAtFrozenDateTime(new \DateTime('2022-05-01'), function () {
            $this->setUp(MockedResponses::FLAGS_V2_RESPONSE, personalApiKey: null);
            $this->assertTrue(PostHog::isFeatureEnabled('simple-test', 'user-id'));
            PostHog::flush();
            $this->assertEquals(
                $this->http_client->calls,
                array(
                0 => array(
                    "path" => "/flags/?v=2",
                    "payload" => sprintf('{"api_key":"%s","distinct_id":"user-id","person_properties":{"distinct_id":"user-id"}}', self::FAKE_API_KEY),
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                        "requestOptions" => array("timeout" => 3000, "shouldRetry" => false),
                    ),
                1 => array(
                    "path" => "/batch/",
                    "payload" => '{"batch":[{"properties":{"$active_feature_flags":[],"$feature_flag":"simple-test","$feature_flag_response":true,"$feature_flag_request_id":"98487c8a-287a-4451-a085-299cd76228dd","$feature_flag_id":6,"$feature_flag_version":1,"$feature_flag_reason":"Matched condition set 1","$lib":"posthog-php","$lib_version":"' . PostHog::VERSION . '","$lib_consumer":"LibCurl","$groups":[]},"distinct_id":"user-id","event":"$feature_flag_called","$groups":[],"library":"posthog-php","library_version":"' . PostHog::VERSION . '","library_consumer":"LibCurl","groups":[],"timestamp":"2022-05-01T00:00:00+00:00","type":"capture"}],"api_key":"random_key"}',
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                    "requestOptions" => array('shouldVerify' => true),
                    ),
                )
            );
        });
    }

    /**
     * @dataProvider decideResponseCases
     */
    public function testIsFeatureEnabledGroups($response)
    {
        $this->setUp($response);
        $this->assertFalse(PostHog::isFeatureEnabled('having_fun', 'user-id', array("company" => "id:5")));

        $this->assertEquals(
            $this->http_client->calls,
            array(
                0 => array(
                    "path" => "/api/feature_flag/local_evaluation?send_cohorts&token=random_key",
                    "payload" => null,
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION, 1 => 'Authorization: Bearer test'),
                    "requestOptions" => array("includeEtag" => true),
                ),
                1 => array(
                    "path" => "/flags/?v=2",
                    "payload" => sprintf(
                        '{"api_key":"%s","distinct_id":"user-id","groups":{"company":"id:5"},"person_properties":{"distinct_id":"user-id"},"group_properties":{"company":{"$group_key":"id:5"}}}',
                        self::FAKE_API_KEY
                    ),
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                    "requestOptions" => array("timeout" => 3000, "shouldRetry" => false),
                ),
            )
        );
    }

    /**
     * @dataProvider decideResponseCases
     */
    public function testGetFeatureFlag($response)
    {
        $this->setUp($response);
        $this->assertEquals("variant-value", PostHog::getFeatureFlag('multivariate-test', 'user-id'));
        $this->assertEquals(
            $this->http_client->calls,
            array(
                0 => array(
                    "path" => "/api/feature_flag/local_evaluation?send_cohorts&token=random_key",
                    "payload" => null,
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION, 1 => 'Authorization: Bearer test'),
                    "requestOptions" => array("includeEtag" => true),
                ),
                1 => array(
                    "path" => "/flags/?v=2",
                    "payload" => sprintf('{"api_key":"%s","distinct_id":"user-id","person_properties":{"distinct_id":"user-id"}}', self::FAKE_API_KEY),
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                    "requestOptions" => array("timeout" => 3000, "shouldRetry" => false),
                ),
            )
        );
    }

    public function testGetFeatureFlagCapturesFeatureFlagCalledEventWithAdditionalMetadata()
    {
        ClockMock::executeAtFrozenDateTime(new \DateTime('2022-05-01'), function () {
            $this->setUp(MockedResponses::FLAGS_V2_RESPONSE, personalApiKey: null);
            $this->assertEquals("variant-value", PostHog::getFeatureFlag('multivariate-test', 'user-id'));
            PostHog::flush();
            $this->assertEquals(
                $this->http_client->calls,
                array(
                0 => array(
                    "path" => "/flags/?v=2",
                    "payload" => sprintf('{"api_key":"%s","distinct_id":"user-id","person_properties":{"distinct_id":"user-id"}}', self::FAKE_API_KEY),
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                    "requestOptions" => array("timeout" => 3000, "shouldRetry" => false),
                ),
                1 => array(
                    "path" => "/batch/",
                    "payload" => '{"batch":[{"properties":{"$active_feature_flags":[],"$feature_flag":"multivariate-test","$feature_flag_response":"variant-value","$feature_flag_request_id":"98487c8a-287a-4451-a085-299cd76228dd","$feature_flag_id":7,"$feature_flag_version":3,"$feature_flag_reason":"Matched condition set 2","$lib":"posthog-php","$lib_version":"' . PostHog::VERSION . '","$lib_consumer":"LibCurl","$groups":[]},"distinct_id":"user-id","event":"$feature_flag_called","$groups":[],"library":"posthog-php","library_version":"' . PostHog::VERSION . '","library_consumer":"LibCurl","groups":[],"timestamp":"2022-05-01T00:00:00+00:00","type":"capture"}],"api_key":"random_key"}',
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                    "requestOptions" => array('shouldVerify' => true),
                ),
                )
            );
        });
    }

    /**
     * @dataProvider decideResponseCases
     */
    public function testGetFeatureFlagDefault($response)
    {
        $this->setUp($response);
        $this->assertEquals(PostHog::getFeatureFlag('blah', 'user-id'), null);

        $this->checkEmptyErrorLogs();
    }

    /**
     * @dataProvider decideResponseCases
     */
    public function testGetFeatureFlagGroups($response)
    {
        $this->setUp($response);
        $this->assertEquals(
            "variant-value",
            PostHog::getFeatureFlag('multivariate-test', 'user-id', array("company" => "id:5"))
        );

        $this->assertEquals(
            $this->http_client->calls,
            array(
                0 => array(
                    "path" => "/api/feature_flag/local_evaluation?send_cohorts&token=random_key",
                    "payload" => null,
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION, 1 => 'Authorization: Bearer test'),
                    "requestOptions" => array("includeEtag" => true),
                ),
                1 => array(
                    "path" => "/flags/?v=2",
                    "payload" => sprintf(
                        '{"api_key":"%s","distinct_id":"user-id","groups":{"company":"id:5"},"person_properties":{"distinct_id":"user-id"},"group_properties":{"company":{"$group_key":"id:5"}}}',
                        self::FAKE_API_KEY
                    ),
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                    "requestOptions" => array("timeout" => 3000, "shouldRetry" => false),
                ),
            )
        );
    }

    /**
     * @dataProvider decideResponseCases
     */
    public function testfetchFeatureVariants($response)
    {
        $this->setUp($response);
        $this->assertIsArray(PostHog::fetchFeatureVariants('user-id'));
    }

     /**
     * @dataProvider decideResponseCases
     */
    public function testGetFeatureFlagPayloadHandlesJson($response): void
    {
        $this->setUp($response);

        $this->assertSame(['key' => 'value'], PostHog::getFeatureFlagPayload('json-payload', 'some-distinct'));
    }

    /**
     * @dataProvider decideResponseCases
     */
    public function testGetFeatureFlagPayloadHandlesIntegers($response): void
    {
        $this->setUp($response);

        $this->assertSame(2500, PostHog::getFeatureFlagPayload('integer-payload', 'some-distinct'));
    }

    /**
     * @dataProvider decideResponseCases
     */
    public function testGetFeatureFlagPayloadHandlesString($response): void
    {
        $this->setUp($response);

        $this->assertSame('A String', PostHog::getFeatureFlagPayload('string-payload', 'some-distinct'));
    }

    /**
     * @dataProvider decideResponseCases
     */
    public function testGetFeatureFlagPayloadHandlesArray($response): void
    {
        $this->setUp($response);

        $this->assertSame([1, 2, 3], PostHog::getFeatureFlagPayload('array-payload', 'some-distinct'));
    }

    /**
     * @dataProvider decideResponseCases
     */
    public function testGetFeatureFlagPayloadHandlesFlagDisabled($response): void
    {
        $this->setUp($response);

        $this->assertNull(PostHog::getFeatureFlagPayload('disabled-flag', 'some-distinct'));
    }

    /**
     * @dataProvider decideResponseCases
     */
    public function testGetFeatureFlagPayloadHandlesFlagNotInResults($response): void
    {
        $this->setUp($response);

        $this->assertNull(PostHog::getFeatureFlagPayload('non-existent-flag', 'some-distinct'));
    }
}
