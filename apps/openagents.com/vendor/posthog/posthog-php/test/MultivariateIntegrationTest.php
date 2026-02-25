<?php

namespace PostHog\Test;

use PHPUnit\Framework\TestCase;
use PostHog\Client;

class MultivariateIntegrationTest extends TestCase
{
    public function testMultivariateFlagDependencies(): void
    {
        // Create a client with mock multivariate flags that have dependencies
        $client = new Client("fake-api-key", [], null, null, false);

        // Leaf flag: multivariate with consistent hashing
        $leafFlag = [
            "id" => 1,
            "name" => "Leaf Multivariate Flag",
            "key" => "leaf-mv-flag",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [],
                        "rollout_percentage" => 100,
                    ]
                ],
                "multivariate" => [
                    "variants" => [
                        ["key" => "variant-a", "name" => "Variant A", "rollout_percentage" => 33],
                        ["key" => "variant-b", "name" => "Variant B", "rollout_percentage" => 33],
                        ["key" => "variant-c", "name" => "Variant C", "rollout_percentage" => 34]
                    ]
                ]
            ]
        ];

        // Dependent flag: depends on specific variant of leaf flag
        $dependentFlag = [
            "id" => 2,
            "name" => "Multivariate Dependent Flag",
            "key" => "dependent-mv-flag",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            [
                                "key" => "leaf-mv-flag",
                                "operator" => "flag_evaluates_to",
                                "value" => "variant-a", // Only true when leaf flag returns variant-a
                                "type" => "flag",
                                "dependency_chain" => ["leaf-mv-flag"],
                            ]
                        ],
                        "rollout_percentage" => 100,
                        "variant" => "special-variant",
                    ]
                ],
                "multivariate" => [
                    "variants" => [
                        ["key" => "special-variant", "name" => "Special Variant", "rollout_percentage" => 100]
                    ]
                ]
            ]
        ];

        $client->featureFlags = [$leafFlag, $dependentFlag];
        $client->featureFlagsByKey = [
            "leaf-mv-flag" => $leafFlag,
            "dependent-mv-flag" => $dependentFlag
        ];

        // Test with different user IDs to get different variants
        // We'll test a few different user IDs until we find one that gets variant-a
        $foundVariantA = false;
        $foundOtherVariant = false;

        for ($i = 0; $i < 100; $i++) {
            $userId = "test-user-{$i}";

            $leafResult = $client->getFeatureFlag(
                "leaf-mv-flag",
                $userId,
                [],
                [],
                [],
                true // only_evaluate_locally
            );

            $dependentResult = $client->getFeatureFlag(
                "dependent-mv-flag",
                $userId,
                [],
                [],
                [],
                true // only_evaluate_locally
            );

            if ($leafResult === "variant-a") {
                // When leaf flag is variant-a, dependent should be "special-variant"
                $this->assertEquals("special-variant", $dependentResult);
                $foundVariantA = true;
            } else {
                // When leaf flag is NOT variant-a, dependent should be false
                $this->assertFalse($dependentResult);
                $foundOtherVariant = true;
            }

            if ($foundVariantA && $foundOtherVariant) {
                break; // We've tested both cases
            }
        }

        // Make sure we tested both scenarios
        $this->assertTrue($foundVariantA, "Should have found at least one user that gets variant-a");
        $this->assertTrue($foundOtherVariant, "Should have found at least one user that gets other variants");
    }

    public function testBooleanFlagDependencyOnMultivariate(): void
    {
        // Test a boolean flag that depends on any variant of a multivariate flag
        $client = new Client("fake-api-key", [], null, null, false);

        $multivariateFlag = [
            "id" => 1,
            "name" => "Multivariate Base",
            "key" => "mv-base",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [],
                        "rollout_percentage" => 100,
                    ]
                ],
                "multivariate" => [
                    "variants" => [
                        ["key" => "control", "name" => "Control", "rollout_percentage" => 50],
                        ["key" => "test", "name" => "Test", "rollout_percentage" => 50]
                    ]
                ]
            ]
        ];

        $booleanFlag = [
            "id" => 2,
            "name" => "Boolean Dependent",
            "key" => "boolean-dependent",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            [
                                "key" => "mv-base",
                                "operator" => "flag_evaluates_to",
                                "value" => true, // Any variant should satisfy this
                                "type" => "flag",
                                "dependency_chain" => ["mv-base"],
                            ]
                        ],
                        "rollout_percentage" => 100,
                    ]
                ]
            ]
        ];

        $client->featureFlags = [$multivariateFlag, $booleanFlag];
        $client->featureFlagsByKey = [
            "mv-base" => $multivariateFlag,
            "boolean-dependent" => $booleanFlag
        ];

        // Test several users - all should have boolean flag true since multivariate always returns a variant
        for ($i = 0; $i < 10; $i++) {
            $userId = "user-{$i}";

            $mvResult = $client->getFeatureFlag("mv-base", $userId, [], [], [], true);
            $booleanResult = $client->getFeatureFlag("boolean-dependent", $userId, [], [], [], true);

            // Multivariate should always return either "control" or "test"
            $this->assertContains($mvResult, ["control", "test"]);

            // Boolean dependent should always be true because any variant satisfies boolean true
            $this->assertTrue($booleanResult);
        }
    }
}
