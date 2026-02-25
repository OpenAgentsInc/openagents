<?php

namespace PostHog\Test;

use PHPUnit\Framework\TestCase;
use PostHog\Client;

class FlagDependencyIntegrationTest extends TestCase
{
    public function testClientIntegrationWithFlagDependencies(): void
    {
        // Create a client with mock flags that have dependencies
        $client = new Client("fake-api-key", [], null, null, false);
        // Set up flags manually (simulating what would come from the API)
        $client->featureFlags = [
            [
                "id" => 1,
                "name" => "Base Flag",
                "key" => "base-flag",
                "active" => true,
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "email",
                                    "operator" => "icontains",
                                    "value" => "@company.com",
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 100,
                        ]
                    ]
                ]
            ],
            [
                "id" => 2,
                "name" => "Dependent Flag",
                "key" => "dependent-flag",
                "active" => true,
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "base-flag",
                                    "operator" => "flag_evaluates_to",
                                    "value" => true,
                                    "type" => "flag",
                                    "dependency_chain" => ["base-flag"],
                                ],
                                [
                                    "key" => "role",
                                    "operator" => "exact",
                                    "value" => "admin",
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 100,
                        ]
                    ]
                ]
            ]
        ];

        // Build flags by key dictionary
        $client->featureFlagsByKey = [];
        foreach ($client->featureFlags as $flag) {
            $client->featureFlagsByKey[$flag['key']] = $flag;
        }

        // Test 1: When both base flag and dependent conditions are satisfied
        $result = $client->getFeatureFlag(
            "dependent-flag",
            "test-user",
            [],
            ["email" => "admin@company.com", "role" => "admin"],
            [],
            true // only_evaluate_locally
        );
        $this->assertTrue($result);

        // Test 2: When base flag condition is satisfied but dependent condition is not
        $result = $client->getFeatureFlag(
            "dependent-flag",
            "test-user-2",
            [],
            ["email" => "user@company.com", "role" => "user"], // role is not admin
            [],
            true // only_evaluate_locally
        );
        $this->assertFalse($result);

        // Test 3: When base flag condition is not satisfied
        $result = $client->getFeatureFlag(
            "dependent-flag",
            "test-user-3",
            [],
            ["email" => "user@external.com", "role" => "admin"], // email domain doesn't match
            [],
            true // only_evaluate_locally
        );
        $this->assertFalse($result);

        // Test 4: Test getAllFlags with dependencies
        $allFlags = $client->getAllFlags(
            "test-user",
            [],
            ["email" => "admin@company.com", "role" => "admin"],
            [],
            true // only_evaluate_locally
        );

        $this->assertTrue($allFlags["base-flag"]);
        $this->assertTrue($allFlags["dependent-flag"]);

        // Test 5: Test getAllFlags when dependency fails
        $allFlags = $client->getAllFlags(
            "test-user-external",
            [],
            ["email" => "admin@external.com", "role" => "admin"], // email domain doesn't match base flag
            [],
            true // only_evaluate_locally
        );

        $this->assertFalse($allFlags["base-flag"]);
        $this->assertFalse($allFlags["dependent-flag"]);
    }
}
