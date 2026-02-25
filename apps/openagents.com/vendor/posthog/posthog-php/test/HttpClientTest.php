<?php
// phpcs:ignoreFile
namespace PostHog\Test;

use PHPUnit\Framework\TestCase;
use PostHog\HttpClient;

class HttpClientTest extends TestCase
{
    public function testMaskTokensInUrl(): void
    {
        $httpClient = new HttpClient("app.posthog.com");

        // Test masking token in middle of URL
        $url = 'https://example.com/api/flags?token=phc_abc123xyz789&send_cohorts';
        $result = $httpClient->maskTokensInUrl($url);
        $this->assertEquals('https://example.com/api/flags?token=[REDACTED]&send_cohorts', $result);

        // Test masking token at end of URL
        $url = 'https://example.com/api/flags?token=phc_abc123xyz789';
        $result = $httpClient->maskTokensInUrl($url);
        $this->assertEquals('https://example.com/api/flags?token=[REDACTED]', $result);

        // Test URL without token
        $url = 'https://example.com/api/flags?other=value';
        $result = $httpClient->maskTokensInUrl($url);
        $this->assertEquals('https://example.com/api/flags?other=value', $result);

        // Test short token - should still be redacted
        $url = 'https://example.com/api/flags?token=short';
        $result = $httpClient->maskTokensInUrl($url);
        $this->assertEquals('https://example.com/api/flags?token=[REDACTED]', $result);

        // Test empty token value
        $url = 'https://example.com/api/flags?token=&other=value';
        $result = $httpClient->maskTokensInUrl($url);
        $this->assertEquals('https://example.com/api/flags?token=&other=value', $result);
    }
}
