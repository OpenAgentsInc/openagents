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


class PostHogTest extends TestCase
{
    const FAKE_API_KEY = "random_key";

    private $http_client;
    private $client;

    public function setUp(): void
    {
        date_default_timezone_set("UTC");
        $this->http_client = new MockedHttpClient("app.posthog.com");
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        // Reset the errorMessages array before each test
        global $errorMessages;
        $errorMessages = [];
    }

    public function checkEmptyErrorLogs(): void
    {
        global $errorMessages;
        $this->assertEmpty($errorMessages);
    }

    public function testInitWithParamApiKey(): void
    {
        $this->expectNotToPerformAssertions();

        PostHog::init("BrpS4SctoaCCsyjlnlun3OzyNJAafdlv__jUWaaJWXg", array("debug" => true));
    }

    public function testInitWithEnvApiKey(): void
    {
        $this->expectNotToPerformAssertions();
        putenv(PostHog::ENV_API_KEY . "=BrpS4SctoaCCsyjlnlun3OzyNJAafdlv__jUWaaJWXg");
        PostHog::init(null, array("debug" => true));

        // Clear the environment variable
        putenv(PostHog::ENV_API_KEY);
    }

    public function testInitThrowsExceptionWithNoApiKey(): void
    {
        $this->expectException(Exception::class);
        $this->expectExceptionMessage("PostHog::init() requires an apiKey");
        PostHog::init(null);
    }

    public function testCapture(): void
    {
        self::assertTrue(
            PostHog::capture(
                array(
                    "distinctId" => "john",
                    "event" => "Module PHP Event",
                )
            )
        );
    }

    public function testCaptureWithSendFeatureFlagsOption(): void
    {
        ClockMock::executeAtFrozenDateTime(new \DateTime('2022-05-01'), function () {
            $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_MULTIPLE_REQUEST);
            $this->client = new Client(
                self::FAKE_API_KEY,
                [
                    "debug" => true,
                    "feature_flag_request_timeout_ms" => 1234,
                ],
                $this->http_client,
                "test"
            );
            PostHog::init(null, null, $this->client);

            $this->assertTrue(
                PostHog::capture(
                    array (
                        "distinctId" => "john",
                        "event" => "Module PHP Event",
                        "send_feature_flags" => true
                    )
                )
            );
            PostHog::flush();

            $this->assertEquals(
                $this->http_client->calls,
                array (
                    0 => array (
                        "path" => "/api/feature_flag/local_evaluation?send_cohorts&token=random_key",
                        "payload" => null,
                        "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION, 1 => 'Authorization: Bearer test'),
                        "requestOptions" => array("includeEtag" => true),
                    ),
                    1 => array (
                        "path" => "/flags/?v=2",
                        "payload" => sprintf('{"api_key":"%s","distinct_id":"john"}', self::FAKE_API_KEY),
                        "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                        "requestOptions" => array("timeout" => 1234, "shouldRetry" => false),
                    ),
                    2 => array (
                        "path" => "/batch/",
                        "payload" => '{"batch":[{"event":"Module PHP Event","send_feature_flags":true,"properties":{"$feature\/simpleFlag":true,"$feature\/having_fun":false,"$feature\/enabled-flag":true,"$feature\/disabled-flag":false,"$feature\/multivariate-simple-test":"variant-simple-value","$feature\/simple-test":true,"$feature\/multivariate-test":"variant-value","$feature\/group-flag":"decide-fallback-value","$feature\/complex-flag":"decide-fallback-value","$feature\/beta-feature":"decide-fallback-value","$feature\/beta-feature2":"alakazam","$feature\/feature-1":"decide-fallback-value","$feature\/feature-2":"decide-fallback-value","$feature\/variant-1":"variant-1","$feature\/variant-3":"variant-3","$active_feature_flags":["simpleFlag","enabled-flag","multivariate-simple-test","simple-test","multivariate-test","group-flag","complex-flag","beta-feature","beta-feature2","feature-1","feature-2","variant-1","variant-3"],"$lib":"posthog-php","$lib_version":"' . PostHog::VERSION . '","$lib_consumer":"LibCurl"},"library":"posthog-php","library_version":"' . PostHog::VERSION . '","library_consumer":"LibCurl","distinct_id":"john","groups":[],"timestamp":"2022-05-01T00:00:00+00:00","type":"capture"}],"api_key":"random_key"}',
                        "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                        "requestOptions" => array('shouldVerify' => true),
                    ),
                )
            );

            // check true-flag is not in captured event
            $this->assertEquals(
                strpos($this->http_client->calls[2]["payload"], 'simpleFlag'),
                true
            );
            $this->assertEquals(
                strpos($this->http_client->calls[2]["payload"], 'true-flag'),
                false
            );
        });
    }

    public function testCaptureWithLocalSendFlags(): void
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_MULTIPLE_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        ClockMock::executeAtFrozenDateTime(new \DateTime('2022-05-01'), function () {
            $this->assertTrue(
                PostHog::capture(
                    array (
                        "distinctId" => "john",
                        "event" => "Module PHP Event",
                    )
                )
            );

            PostHog::flush();

            $this->assertEquals(
                $this->http_client->calls,
                array (
                    0 => array (
                        "path" => "/api/feature_flag/local_evaluation?send_cohorts&token=random_key",
                        "payload" => null,
                        "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION, 1 => 'Authorization: Bearer test'),
                        "requestOptions" => array("includeEtag" => true),
                    ),
                    1 => array (
                        "path" => "/batch/",
                        "payload" => '{"batch":[{"event":"Module PHP Event","properties":{"$feature\/true-flag":true,"$active_feature_flags":["true-flag"],"$lib":"posthog-php","$lib_version":"' . PostHog::VERSION . '","$lib_consumer":"LibCurl"},"library":"posthog-php","library_version":"' . PostHog::VERSION . '","library_consumer":"LibCurl","distinct_id":"john","groups":[],"timestamp":"2022-05-01T00:00:00+00:00","type":"capture"}],"api_key":"random_key"}',
                        "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                        "requestOptions" => array('shouldVerify' => true),
                    ),
                )
            );
        });
    }

    public function testCaptureWithLocalSendFlagsNoOverrides(): void
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com", flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_MULTIPLE_REQUEST);
        $this->client = new Client(
            self::FAKE_API_KEY,
            [
                "debug" => true,
            ],
            $this->http_client,
            "test"
        );
        PostHog::init(null, null, $this->client);

        ClockMock::executeAtFrozenDateTime(new \DateTime('2022-05-01'), function () {
            $this->assertTrue(
                PostHog::capture(
                    array (
                        "distinctId" => "john",
                        "event" => "Module PHP Event",
                        "properties" => array (
                            "\$feature/true-flag" => "random-override"
                        )
                    )
                )
            );

            PostHog::flush();

            $this->assertEquals(
                $this->http_client->calls,
                array (
                    0 => array (
                        "path" => "/api/feature_flag/local_evaluation?send_cohorts&token=random_key",
                        "payload" => null,
                        "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION, 1 => 'Authorization: Bearer test'),
                        "requestOptions" => array("includeEtag" => true),

                    ),
                    1 => array (
                        "path" => "/batch/",
                        "payload" => '{"batch":[{"event":"Module PHP Event","properties":{"$feature\/true-flag":"random-override","$active_feature_flags":["true-flag"],"$lib":"posthog-php","$lib_version":"' . PostHog::VERSION . '","$lib_consumer":"LibCurl"},"library":"posthog-php","library_version":"' . PostHog::VERSION . '","library_consumer":"LibCurl","distinct_id":"john","groups":[],"timestamp":"2022-05-01T00:00:00+00:00","type":"capture"}],"api_key":"random_key"}',
                        "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                        "requestOptions" => array('shouldVerify' => true),
                    ),
                )
            );
        });
    }

    public function testIdentify(): void
    {
        self::assertTrue(
            PostHog::identify(
                array(
                    "distinctId" => "doe",
                    "properties" => array(
                        "loves_php" => false,
                        "birthday" => time(),
                    ),
                )
            )
        );
    }

    public function testEmptyProperties(): void
    {
        self::assertTrue(
            PostHog::identify(
                array(
                    "distinctId" => "empty-properties",
                )
            )
        );

        self::assertTrue(
            PostHog::capture(
                array(
                    "distinctId" => "user-id",
                    "event" => "empty-properties",
                )
            )
        );
    }

    public function testEmptyArrayProperties(): void
    {
        self::assertTrue(
            PostHog::identify(
                array(
                    "distinctId" => "empty-properties",
                    "properties" => array(),
                )
            )
        );

        self::assertTrue(
            PostHog::capture(
                array(
                    "distinctId" => "user-id",
                    "event" => "empty-properties",
                    "properties" => array(),
                )
            )
        );
    }

    public function testAlias(): void
    {
        self::assertTrue(
            PostHog::alias(
                array(
                    "alias" => "previous-id",
                    "distinctId" => "user-id",
                )
            )
        );
    }

    public function testTimestamps(): void
    {
        self::assertTrue(
            PostHog::capture(
                array(
                    "distinctId" => "user-id",
                    "event" => "integer-timestamp",
                    "timestamp" => (int) mktime(0, 0, 0, date('n'), 1, date('Y')),
                )
            )
        );

        self::assertTrue(
            PostHog::capture(
                array(
                    "distinctId" => "user-id",
                    "event" => "string-integer-timestamp",
                    "timestamp" => (string) mktime(0, 0, 0, date('n'), 1, date('Y')),
                )
            )
        );

        self::assertTrue(
            PostHog::capture(
                array(
                    "distinctId" => "user-id",
                    "event" => "iso8630-timestamp",
                    "timestamp" => date(DATE_ATOM, mktime(0, 0, 0, date('n'), 1, date('Y'))),
                )
            )
        );

        self::assertTrue(
            PostHog::capture(
                array(
                    "distinctId" => "user-id",
                    "event" => "iso8601-timestamp",
                    "timestamp" => date(DATE_ATOM, mktime(0, 0, 0, date('n'), 1, date('Y'))),
                )
            )
        );

        self::assertTrue(
            PostHog::capture(
                array(
                    "distinctId" => "user-id",
                    "event" => "strtotime-timestamp",
                    "timestamp" => strtotime('1 week ago'),
                )
            )
        );
    }

    public function testGroupIdentify(): void
    {
        self::assertTrue(
            PostHog::groupIdentify(
                array(
                    "groupType" => "company",
                    "groupKey" => "id:5",
                    "properties" => array(
                        "foo" => "bar"
                    )
                )
            )
        );

        self::assertTrue(
            PostHog::groupIdentify(
                array(
                    "groupType" => "company",
                    "groupKey" => "id:5",
                )
            )
        );
    }

    public function testGroupIdentifyValidation(): void
    {
        try {
            Posthog::groupIdentify(array());
        } catch (Exception $e) {
            $this->assertEquals("PostHog::groupIdentify() expects a groupType", $e->getMessage());
        }
    }

    public function testDefaultPropertiesGetAddedProperly(): void
    {
        PostHog::getFeatureFlag('random_key', 'some_id', array("company" => "id:5", "instance" => "app.posthog.com"), array("x1" => "y1"), array("company" => array("x" => "y")));
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
                    "payload" => sprintf('{"api_key":"%s","distinct_id":"some_id","groups":{"company":"id:5","instance":"app.posthog.com"},"person_properties":{"distinct_id":"some_id","x1":"y1"},"group_properties":{"company":{"$group_key":"id:5","x":"y"},"instance":{"$group_key":"app.posthog.com"}}}', self::FAKE_API_KEY),
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                    "requestOptions" => array("timeout" => 3000, "shouldRetry" => false),
                ),
            )
        );

        // reset calls
        $this->http_client->calls = array();

        PostHog::getFeatureFlag(
            'random_key',
            'some_id',
            array("company" => "id:5", "instance" => "app.posthog.com"),
            array("distinct_id" => "override"),
            array("company" => array("\$group_key" => "group_override"), "instance" => array("\$group_key" => "app.posthog.com"))
        );
        $this->assertEquals(
            $this->http_client->calls,
            array(
                0 => array(
                    "path" => "/flags/?v=2",
                    "payload" => sprintf('{"api_key":"%s","distinct_id":"some_id","groups":{"company":"id:5","instance":"app.posthog.com"},"person_properties":{"distinct_id":"override"},"group_properties":{"company":{"$group_key":"group_override"},"instance":{"$group_key":"app.posthog.com"}}}', self::FAKE_API_KEY),
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                    "requestOptions" => array("timeout" => 3000, "shouldRetry" => false),
                ),
            )
        );
        // reset calls
        $this->http_client->calls = array();

        # test empty
        PostHog::getFeatureFlag('random_key', 'some_id', array("company" => "id:5"), [], []);
        $this->assertEquals(
            $this->http_client->calls,
            array(
                0 => array(
                    "path" => "/flags/?v=2",
                    "payload" => sprintf('{"api_key":"%s","distinct_id":"some_id","groups":{"company":"id:5"},"person_properties":{"distinct_id":"some_id"},"group_properties":{"company":{"$group_key":"id:5"}}}', self::FAKE_API_KEY),
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                    "requestOptions" => array("timeout" => 3000, "shouldRetry" => false),
                ),
            )
        );

        // reset calls
        $this->http_client->calls = array();

        PostHog::isFeatureEnabled('random_key', 'some_id', array("company" => "id:5", "instance" => "app.posthog.com"), array("x1" => "y1"), array("company" => array("x" => "y")));
        $this->assertEquals(
            $this->http_client->calls,
            array(
                0 => array(
                    "path" => "/flags/?v=2",
                    "payload" => sprintf('{"api_key":"%s","distinct_id":"some_id","groups":{"company":"id:5","instance":"app.posthog.com"},"person_properties":{"distinct_id":"some_id","x1":"y1"},"group_properties":{"company":{"$group_key":"id:5","x":"y"},"instance":{"$group_key":"app.posthog.com"}}}', self::FAKE_API_KEY),
                    "extraHeaders" => array(0 => 'User-Agent: posthog-php/' . PostHog::VERSION),
                    "requestOptions" => array("timeout" => 3000, "shouldRetry" => false),
                ),
            )
        );
    }
}
