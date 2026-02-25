<?php
// phpcs:ignoreFile
namespace PostHog\Test;

// comment out below to print all logs instead of failing tests
require_once 'test/error_log_mock.php';

use PHPUnit\Framework\TestCase;
use PostHog\Client;
use PostHog\PostHog;
use PostHog\Test\Assets\MockedResponses;

class EtagSupportTest extends TestCase
{
    protected const FAKE_API_KEY = "random_key";

    protected Client $client;
    protected MockedHttpClient $http_client;

    public function setUp(): void
    {
        date_default_timezone_set("UTC");

        // Reset the errorMessages array before each test
        global $errorMessages;
        $errorMessages = [];
    }

    public function checkEmptyErrorLogs(): void
    {
        global $errorMessages;
        $this->assertTrue(empty($errorMessages), "Error logs are not empty: " . implode("\n", $errorMessages));
    }

    public function testStoresEtagFromInitialResponse(): void
    {
        $this->http_client = new MockedHttpClient(
            host: "app.posthog.com",
            flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_REQUEST,
            flagEndpointEtag: '"abc123"'
        );

        $this->client = new Client(
            self::FAKE_API_KEY,
            [],
            $this->http_client,
            "test"
        );

        $this->assertEquals('"abc123"', $this->client->getFlagsEtag());
        $this->assertCount(1, $this->client->featureFlags);
        $this->checkEmptyErrorLogs();
    }

    public function testSendsIfNoneMatchHeaderOnSubsequentRequests(): void
    {
        $this->http_client = new MockedHttpClient(
            host: "app.posthog.com",
            flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_REQUEST,
            flagEndpointEtag: '"initial-etag"'
        );

        $this->client = new Client(
            self::FAKE_API_KEY,
            [],
            $this->http_client,
            "test"
        );

        // First call sets the ETag
        $this->assertEquals('"initial-etag"', $this->client->getFlagsEtag());

        // Set up queue for second call to return 304
        $this->http_client->setFlagEndpointResponseQueue([
            ['response' => [], 'etag' => '"initial-etag"', 'responseCode' => 304]
        ]);

        // Reload flags - should send If-None-Match
        $this->client->loadFlags();

        // Check that If-None-Match was sent in the second request
        $calls = $this->http_client->calls;
        $this->assertCount(2, $calls);

        // First call should not have If-None-Match
        $firstCallHeaders = $calls[0]['extraHeaders'];
        $hasIfNoneMatch = false;
        foreach ($firstCallHeaders as $header) {
            if (str_starts_with($header, 'If-None-Match:')) {
                $hasIfNoneMatch = true;
                break;
            }
        }
        $this->assertFalse($hasIfNoneMatch, "First call should not have If-None-Match header");

        // Second call should have If-None-Match
        $secondCallHeaders = $calls[1]['extraHeaders'];
        $foundIfNoneMatch = false;
        foreach ($secondCallHeaders as $header) {
            if (str_starts_with($header, 'If-None-Match:')) {
                $foundIfNoneMatch = true;
                $this->assertEquals('If-None-Match: "initial-etag"', $header);
                break;
            }
        }
        $this->assertTrue($foundIfNoneMatch, "Second call should have If-None-Match header");

        $this->checkEmptyErrorLogs();
    }

    public function testHandles304NotModifiedAndPreservesCachedFlags(): void
    {
        $this->http_client = new MockedHttpClient(
            host: "app.posthog.com",
            flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_REQUEST,
            flagEndpointEtag: '"test-etag"'
        );

        $this->client = new Client(
            self::FAKE_API_KEY,
            [],
            $this->http_client,
            "test"
        );

        // Verify initial flags are loaded
        $this->assertCount(1, $this->client->featureFlags);
        $this->assertEquals('person-flag', $this->client->featureFlags[0]['key']);

        // Set up queue for second call to return 304
        $this->http_client->setFlagEndpointResponseQueue([
            ['response' => [], 'etag' => '"test-etag"', 'responseCode' => 304]
        ]);

        // Reload flags - should get 304
        $this->client->loadFlags();

        // Flags should still be the same (not cleared)
        $this->assertCount(1, $this->client->featureFlags);
        $this->assertEquals('person-flag', $this->client->featureFlags[0]['key']);

        $this->checkEmptyErrorLogs();
    }

    public function testUpdatesEtagWhenFlagsChange(): void
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com");

        // Use queue to provide different responses
        $this->http_client->setFlagEndpointResponseQueue([
            [
                'response' => MockedResponses::LOCAL_EVALUATION_REQUEST,
                'etag' => '"etag-v1"',
                'responseCode' => 200
            ],
            [
                'response' => [
                    'flags' => [['id' => 2, 'key' => 'newFlag', 'active' => true, 'filters' => []]],
                    'group_type_mapping' => []
                ],
                'etag' => '"etag-v2"',
                'responseCode' => 200
            ]
        ]);

        $this->client = new Client(
            self::FAKE_API_KEY,
            [],
            $this->http_client,
            "test"
        );

        $this->assertEquals('"etag-v1"', $this->client->getFlagsEtag());
        $this->assertEquals('person-flag', $this->client->featureFlags[0]['key']);

        $this->client->loadFlags();

        $this->assertEquals('"etag-v2"', $this->client->getFlagsEtag());
        $this->assertEquals('newFlag', $this->client->featureFlags[0]['key']);

        $this->checkEmptyErrorLogs();
    }

    public function testClearsEtagWhenServerStopsSendingIt(): void
    {
        $this->http_client = new MockedHttpClient(host: "app.posthog.com");

        // Use queue to provide different responses
        $this->http_client->setFlagEndpointResponseQueue([
            [
                'response' => MockedResponses::LOCAL_EVALUATION_REQUEST,
                'etag' => '"etag-v1"',
                'responseCode' => 200
            ],
            [
                'response' => [
                    'flags' => [['id' => 2, 'key' => 'newFlag', 'active' => true, 'filters' => []]],
                    'group_type_mapping' => []
                ],
                'etag' => null, // No ETag
                'responseCode' => 200
            ]
        ]);

        $this->client = new Client(
            self::FAKE_API_KEY,
            [],
            $this->http_client,
            "test"
        );

        $this->assertEquals('"etag-v1"', $this->client->getFlagsEtag());

        $this->client->loadFlags();

        $this->assertNull($this->client->getFlagsEtag());
        $this->assertEquals('newFlag', $this->client->featureFlags[0]['key']);

        $this->checkEmptyErrorLogs();
    }

    public function testHandles304WithoutEtagHeaderAndPreservesExistingEtag(): void
    {
        $this->http_client = new MockedHttpClient(
            host: "app.posthog.com",
            flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_REQUEST,
            flagEndpointEtag: '"original-etag"'
        );

        $this->client = new Client(
            self::FAKE_API_KEY,
            [],
            $this->http_client,
            "test"
        );

        $this->assertEquals('"original-etag"', $this->client->getFlagsEtag());

        // Set up queue for second call to return 304 without ETag
        $this->http_client->setFlagEndpointResponseQueue([
            ['response' => [], 'etag' => null, 'responseCode' => 304]
        ]);

        $this->client->loadFlags();

        // ETag should be preserved since server returned 304 (even without new ETag)
        $this->assertEquals('"original-etag"', $this->client->getFlagsEtag());
        // And flags should be preserved
        $this->assertCount(1, $this->client->featureFlags);

        $this->checkEmptyErrorLogs();
    }

    public function testUpdatesEtagWhen304ResponseIncludesNewEtag(): void
    {
        $this->http_client = new MockedHttpClient(
            host: "app.posthog.com",
            flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_REQUEST,
            flagEndpointEtag: '"original-etag"'
        );

        $this->client = new Client(
            self::FAKE_API_KEY,
            [],
            $this->http_client,
            "test"
        );

        $this->assertEquals('"original-etag"', $this->client->getFlagsEtag());

        // Set up queue for second call to return 304 with new ETag
        $this->http_client->setFlagEndpointResponseQueue([
            ['response' => [], 'etag' => '"updated-etag"', 'responseCode' => 304]
        ]);

        $this->client->loadFlags();

        // ETag should be updated to the new value from 304 response
        $this->assertEquals('"updated-etag"', $this->client->getFlagsEtag());
        // And flags should be preserved
        $this->assertCount(1, $this->client->featureFlags);

        $this->checkEmptyErrorLogs();
    }

    public function testProcessesErrorResponseWithoutFlagsKey(): void
    {
        // This test verifies current behavior: error responses without a 'flags' key
        // will result in empty flags (due to $payload['flags'] ?? [])
        // This is pre-existing behavior that's consistent with or without ETag support

        $this->http_client = new MockedHttpClient(
            host: "app.posthog.com",
            flagEndpointResponse: MockedResponses::LOCAL_EVALUATION_REQUEST,
            flagEndpointEtag: '"original-etag"'
        );

        $this->client = new Client(
            self::FAKE_API_KEY,
            [],
            $this->http_client,
            "test"
        );

        $this->assertEquals('"original-etag"', $this->client->getFlagsEtag());
        $this->assertCount(1, $this->client->featureFlags);
        $this->assertEquals('person-flag', $this->client->featureFlags[0]['key']);

        // Set up queue for second call to return 500 error with no flags key
        $this->http_client->setFlagEndpointResponseQueue([
            ['response' => ['error' => 'Internal Server Error'], 'etag' => null, 'responseCode' => 500]
        ]);

        // loadFlags will parse the response and set featureFlags to []
        // This is pre-existing behavior: error responses clear flags
        $this->client->loadFlags();

        // Flags are cleared because response doesn't have 'flags' key
        // ($payload['flags'] ?? [] evaluates to [])
        $this->assertCount(0, $this->client->featureFlags);

        // ETag is set to null (from the response)
        $this->assertNull($this->client->getFlagsEtag());
    }
}
