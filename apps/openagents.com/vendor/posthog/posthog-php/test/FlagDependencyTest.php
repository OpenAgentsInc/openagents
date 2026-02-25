<?php

namespace PostHog\Test;

use PHPUnit\Framework\TestCase;
use PostHog\Client;
use PostHog\FeatureFlag;
use PostHog\InconclusiveMatchException;

class FlagDependencyTest extends TestCase
{
    public function testFlagDependencySimpleChain(): void
    {
        // Test basic flag dependency: flag-b depends on flag-a
        $flagA = [
            "id" => 1,
            "name" => "Flag A",
            "key" => "flag-a",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            ["key" => "email", "operator" => "icontains", "value" => "@example.com", "type" => "person"]
                        ],
                        "rollout_percentage" => 100,
                    ]
                ]
            ]
        ];

        $flagB = [
            "id" => 2,
            "name" => "Flag B",
            "key" => "flag-b",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            [
                                "key" => "flag-a",
                                "operator" => "flag_evaluates_to",
                                "value" => true,
                                "type" => "flag",
                                "dependency_chain" => ["flag-a"],
                            ]
                        ],
                        "rollout_percentage" => 100,
                    ]
                ]
            ]
        ];

        $flagsByKey = [
            "flag-a" => $flagA,
            "flag-b" => $flagB
        ];

        $evaluationCache = [];

        // Test when dependency is satisfied
        $result = FeatureFlag::matchFeatureFlagProperties(
            $flagB,
            "test-user",
            ["email" => "test@example.com"],
            [],
            $flagsByKey,
            $evaluationCache
        );
        $this->assertTrue($result);

        // Test when dependency is not satisfied
        $evaluationCache = []; // Reset cache
        $result = FeatureFlag::matchFeatureFlagProperties(
            $flagB,
            "test-user-2",
            ["email" => "test@other.com"],
            [],
            $flagsByKey,
            $evaluationCache
        );
        $this->assertFalse($result);
    }

    public function testFlagDependencyCircularDependency(): void
    {
        // Test circular dependency handling: flag-a depends on flag-b, flag-b depends on flag-a
        $flagA = [
            "id" => 1,
            "name" => "Flag A",
            "key" => "flag-a",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            [
                                "key" => "flag-b",
                                "operator" => "flag_evaluates_to",
                                "value" => true,
                                "type" => "flag",
                                "dependency_chain" => [], // Empty chain indicates circular dependency
                            ]
                        ],
                        "rollout_percentage" => 100,
                    ]
                ]
            ]
        ];

        $flagB = [
            "id" => 2,
            "name" => "Flag B",
            "key" => "flag-b",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            [
                                "key" => "flag-a",
                                "operator" => "flag_evaluates_to",
                                "value" => true,
                                "type" => "flag",
                                "dependency_chain" => [], // Empty chain indicates circular dependency
                            ]
                        ],
                        "rollout_percentage" => 100,
                    ]
                ]
            ]
        ];

        $flagsByKey = [
            "flag-a" => $flagA,
            "flag-b" => $flagB
        ];

        $evaluationCache = [];

        // Both flags should raise InconclusiveMatchException due to circular dependency
        $this->expectException(InconclusiveMatchException::class);
        $this->expectExceptionMessage("Circular dependency detected for flag 'flag-b'");
        FeatureFlag::matchFeatureFlagProperties($flagA, "test-user", [], [], $flagsByKey, $evaluationCache);
    }

    public function testFlagDependencyMissingFlag(): void
    {
        // Test handling of missing flag dependency
        $flag = [
            "id" => 1,
            "name" => "Flag A",
            "key" => "flag-a",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            [
                                "key" => "non-existent-flag",
                                "operator" => "flag_evaluates_to",
                                "value" => true,
                                "type" => "flag",
                                "dependency_chain" => ["non-existent-flag"],
                            ]
                        ],
                        "rollout_percentage" => 100,
                    ]
                ]
            ]
        ];

        $flagsByKey = [
            "flag-a" => $flag
        ];

        $evaluationCache = [];

        // Should raise InconclusiveMatchException because dependency doesn't exist
        $this->expectException(InconclusiveMatchException::class);
        $this->expectExceptionMessage(
            "Cannot evaluate flag dependency 'non-existent-flag' - flag not found in local flags"
        );
        FeatureFlag::matchFeatureFlagProperties($flag, "test-user", [], [], $flagsByKey, $evaluationCache);
    }

    public function testFlagDependencyComplexChain(): void
    {
        // Test complex dependency chain: flag-d -> flag-c -> [flag-a, flag-b]
        $flagA = [
            "id" => 1,
            "name" => "Flag A",
            "key" => "flag-a",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            ["key" => "email", "operator" => "icontains", "value" => "@example.com", "type" => "person"]
                        ],
                        "rollout_percentage" => 100,
                    ]
                ]
            ]
        ];

        $flagB = [
            "id" => 2,
            "name" => "Flag B",
            "key" => "flag-b",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            ["key" => "name", "operator" => "exact", "value" => "test", "type" => "person"]
                        ],
                        "rollout_percentage" => 100,
                    ]
                ]
            ]
        ];

        $flagC = [
            "id" => 3,
            "name" => "Flag C",
            "key" => "flag-c",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            [
                                "key" => "flag-a",
                                "operator" => "flag_evaluates_to",
                                "value" => true,
                                "type" => "flag",
                                "dependency_chain" => ["flag-a"],
                            ],
                            [
                                "key" => "flag-b",
                                "operator" => "flag_evaluates_to",
                                "value" => true,
                                "type" => "flag",
                                "dependency_chain" => ["flag-b"],
                            ],
                        ],
                        "rollout_percentage" => 100,
                    ]
                ]
            ]
        ];

        $flagD = [
            "id" => 4,
            "name" => "Flag D",
            "key" => "flag-d",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            [
                                "key" => "flag-c",
                                "operator" => "flag_evaluates_to",
                                "value" => true,
                                "type" => "flag",
                                "dependency_chain" => ["flag-a", "flag-b", "flag-c"],
                            ]
                        ],
                        "rollout_percentage" => 100,
                    ]
                ]
            ]
        ];

        $flagsByKey = [
            "flag-a" => $flagA,
            "flag-b" => $flagB,
            "flag-c" => $flagC,
            "flag-d" => $flagD
        ];

        $evaluationCache = [];

        // Test when all dependencies are satisfied
        $result = FeatureFlag::matchFeatureFlagProperties(
            $flagD,
            "test-user",
            ["email" => "test@example.com", "name" => "test"],
            [],
            $flagsByKey,
            $evaluationCache
        );
        $this->assertTrue($result);

        // Test when one dependency fails
        $evaluationCache = []; // Reset cache
        $result = FeatureFlag::matchFeatureFlagProperties(
            $flagD,
            "test-user-2",
            ["email" => "test@other.com", "name" => "test"], // email doesn't match flag-a
            [],
            $flagsByKey,
            $evaluationCache
        );
        $this->assertFalse($result);
    }

    public function testFlagDependencyMixedConditions(): void
    {
        // Test flag dependency mixed with other property conditions
        $baseFlag = [
            "id" => 1,
            "name" => "Base Flag",
            "key" => "base-flag",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            ["key" => "region", "operator" => "exact", "value" => "us", "type" => "person"]
                        ],
                        "rollout_percentage" => 100,
                    ]
                ]
            ]
        ];

        $mixedFlag = [
            "id" => 2,
            "name" => "Mixed Flag",
            "key" => "mixed-flag",
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
                            ["key" => "email", "operator" => "icontains", "value" => "@example.com", "type" => "person"]
                        ],
                        "rollout_percentage" => 100,
                    ]
                ]
            ]
        ];

        $flagsByKey = [
            "base-flag" => $baseFlag,
            "mixed-flag" => $mixedFlag
        ];

        $evaluationCache = [];

        // Both flag dependency and email condition satisfied
        $result = FeatureFlag::matchFeatureFlagProperties(
            $mixedFlag,
            "test-user",
            ["email" => "test@example.com", "region" => "us"],
            [],
            $flagsByKey,
            $evaluationCache
        );
        $this->assertTrue($result);

        // Flag dependency satisfied but email condition not satisfied
        $evaluationCache = []; // Reset cache
        $result = FeatureFlag::matchFeatureFlagProperties(
            $mixedFlag,
            "test-user-2",
            ["email" => "test@other.com", "region" => "us"],
            [],
            $flagsByKey,
            $evaluationCache
        );
        $this->assertFalse($result);

        // Email condition satisfied but flag dependency not satisfied
        $evaluationCache = []; // Reset cache
        $result = FeatureFlag::matchFeatureFlagProperties(
            $mixedFlag,
            "test-user-3",
            ["email" => "test@example.com", "region" => "eu"], // region doesn't match base-flag
            [],
            $flagsByKey,
            $evaluationCache
        );
        $this->assertFalse($result);
    }

    public function testMatchesDependencyValue(): void
    {
        // Test the matches_dependency_value function logic

        // String variant matches string exactly (case-sensitive)
        $this->assertTrue(FeatureFlag::matchesDependencyValue("control", "control"));
        $this->assertTrue(FeatureFlag::matchesDependencyValue("Control", "Control"));
        $this->assertFalse(FeatureFlag::matchesDependencyValue("control", "Control"));
        $this->assertFalse(FeatureFlag::matchesDependencyValue("Control", "CONTROL"));
        $this->assertFalse(FeatureFlag::matchesDependencyValue("control", "test"));

        // String variant matches boolean true (any variant is truthy)
        $this->assertTrue(FeatureFlag::matchesDependencyValue(true, "control"));
        $this->assertTrue(FeatureFlag::matchesDependencyValue(true, "test"));
        $this->assertFalse(FeatureFlag::matchesDependencyValue(false, "control"));

        // Boolean matches boolean exactly
        $this->assertTrue(FeatureFlag::matchesDependencyValue(true, true));
        $this->assertTrue(FeatureFlag::matchesDependencyValue(false, false));
        $this->assertFalse(FeatureFlag::matchesDependencyValue(false, true));
        $this->assertFalse(FeatureFlag::matchesDependencyValue(true, false));

        // Empty string doesn't match
        $this->assertFalse(FeatureFlag::matchesDependencyValue(true, ""));
        $this->assertFalse(FeatureFlag::matchesDependencyValue("control", ""));

        // Type mismatches
        $this->assertFalse(FeatureFlag::matchesDependencyValue(123, "control"));
        $this->assertFalse(FeatureFlag::matchesDependencyValue("control", 123));
    }

    public function testProductionStyleMultivariateDependencyChain(): void
    {
        // Test production-style multivariate dependency chain:
        // multivariate-root-flag -> multivariate-intermediate-flag -> multivariate-leaf-flag
        $client = new Client("fake-api-key", [], null, "fake-personal-api-key", false);
        $client->featureFlags = [
            // Leaf flag: multivariate with fruit variants
            [
                "id" => 451,
                "name" => "Multivariate Leaf Flag (Base)",
                "key" => "multivariate-leaf-flag",
                "active" => true,
                "rollout_percentage" => 100,
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "email",
                                    "type" => "person",
                                    "value" => ["pineapple@example.com"],
                                    "operator" => "exact",
                                ]
                            ],
                            "rollout_percentage" => 100,
                            "variant" => "pineapple",
                        ],
                        [
                            "properties" => [
                                [
                                    "key" => "email",
                                    "type" => "person",
                                    "value" => ["mango@example.com"],
                                    "operator" => "exact",
                                ]
                            ],
                            "rollout_percentage" => 100,
                            "variant" => "mango",
                        ],
                        [
                            "properties" => [
                                [
                                    "key" => "email",
                                    "type" => "person",
                                    "value" => ["papaya@example.com"],
                                    "operator" => "exact",
                                ]
                            ],
                            "rollout_percentage" => 100,
                            "variant" => "papaya",
                        ],
                        [
                            "properties" => [
                                [
                                    "key" => "email",
                                    "type" => "person",
                                    "value" => ["kiwi@example.com"],
                                    "operator" => "exact",
                                ]
                            ],
                            "rollout_percentage" => 100,
                            "variant" => "kiwi",
                        ],
                        [
                            "properties" => [],
                            "rollout_percentage" => 0, // Force default to false for unknown emails
                        ],
                    ],
                    "multivariate" => [
                        "variants" => [
                            ["key" => "pineapple", "rollout_percentage" => 25],
                            ["key" => "mango", "rollout_percentage" => 25],
                            ["key" => "papaya", "rollout_percentage" => 25],
                            ["key" => "kiwi", "rollout_percentage" => 25],
                        ]
                    ],
                ],
            ],
            // Intermediate flag: multivariate with color variants, depends on fruit
            [
                "id" => 467,
                "name" => "Multivariate Intermediate Flag (Depends on fruit)",
                "key" => "multivariate-intermediate-flag",
                "active" => true,
                "rollout_percentage" => 100,
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "multivariate-leaf-flag",
                                    "type" => "flag",
                                    "value" => "pineapple",
                                    "operator" => "flag_evaluates_to",
                                    "dependency_chain" => ["multivariate-leaf-flag"],
                                ]
                            ],
                            "rollout_percentage" => 100,
                            "variant" => "blue",
                        ],
                        [
                            "properties" => [
                                [
                                    "key" => "multivariate-leaf-flag",
                                    "type" => "flag",
                                    "value" => "mango",
                                    "operator" => "flag_evaluates_to",
                                    "dependency_chain" => ["multivariate-leaf-flag"],
                                ]
                            ],
                            "rollout_percentage" => 100,
                            "variant" => "red",
                        ],
                    ],
                    "multivariate" => [
                        "variants" => [
                            ["key" => "blue", "rollout_percentage" => 100],
                            ["key" => "red", "rollout_percentage" => 0],
                            ["key" => "green", "rollout_percentage" => 0],
                            ["key" => "black", "rollout_percentage" => 0],
                        ]
                    ],
                ],
            ],
            // Root flag: multivariate with show variants, depends on color
            [
                "id" => 468,
                "name" => "Multivariate Root Flag (Depends on color)",
                "key" => "multivariate-root-flag",
                "active" => true,
                "rollout_percentage" => 100,
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "multivariate-intermediate-flag",
                                    "type" => "flag",
                                    "value" => "blue",
                                    "operator" => "flag_evaluates_to",
                                    "dependency_chain" => [
                                        "multivariate-leaf-flag",
                                        "multivariate-intermediate-flag",
                                    ],
                                ]
                            ],
                            "rollout_percentage" => 100,
                            "variant" => "breaking-bad",
                        ],
                        [
                            "properties" => [
                                [
                                    "key" => "multivariate-intermediate-flag",
                                    "type" => "flag",
                                    "value" => "red",
                                    "operator" => "flag_evaluates_to",
                                    "dependency_chain" => [
                                        "multivariate-leaf-flag",
                                        "multivariate-intermediate-flag",
                                    ],
                                ]
                            ],
                            "rollout_percentage" => 100,
                            "variant" => "the-wire",
                        ],
                    ],
                    "multivariate" => [
                        "variants" => [
                            ["key" => "breaking-bad", "rollout_percentage" => 100],
                            ["key" => "the-wire", "rollout_percentage" => 0],
                            ["key" => "game-of-thrones", "rollout_percentage" => 0],
                            ["key" => "the-expanse", "rollout_percentage" => 0],
                        ]
                    ],
                ],
            ],
        ];
        $client->featureFlagsByKey = [];
        foreach ($client->featureFlags as $flag) {
            $client->featureFlagsByKey[$flag['key']] = $flag;
        }

        // Test successful pineapple -> blue -> breaking-bad chain
        $leafResult = $client->getFeatureFlag(
            "multivariate-leaf-flag",
            "test-user",
            [],
            ["email" => "pineapple@example.com"],
            [],
            true
        );
        $intermediateResult = $client->getFeatureFlag(
            "multivariate-intermediate-flag",
            "test-user",
            [],
            ["email" => "pineapple@example.com"],
            [],
            true
        );
        $rootResult = $client->getFeatureFlag(
            "multivariate-root-flag",
            "test-user",
            [],
            ["email" => "pineapple@example.com"],
            [],
            true
        );
        $this->assertEquals("pineapple", $leafResult);
        $this->assertEquals("blue", $intermediateResult);
        $this->assertEquals("breaking-bad", $rootResult);

        // Test successful mango -> red -> the-wire chain
        $mangoLeafResult = $client->getFeatureFlag(
            "multivariate-leaf-flag",
            "test-user",
            [],
            ["email" => "mango@example.com"],
            [],
            true
        );
        $mangoIntermediateResult = $client->getFeatureFlag(
            "multivariate-intermediate-flag",
            "test-user",
            [],
            ["email" => "mango@example.com"],
            [],
            true
        );
        $mangoRootResult = $client->getFeatureFlag(
            "multivariate-root-flag",
            "test-user",
            [],
            ["email" => "mango@example.com"],
            [],
            true
        );
        $this->assertEquals("mango", $mangoLeafResult);
        $this->assertEquals("red", $mangoIntermediateResult);
        $this->assertEquals("the-wire", $mangoRootResult);

        // Test broken chain - user without matching email gets default/false results
        $unknownLeafResult = $client->getFeatureFlag(
            "multivariate-leaf-flag",
            "test-user",
            [],
            ["email" => "unknown@example.com"],
            [],
            true
        );
        $unknownIntermediateResult = $client->getFeatureFlag(
            "multivariate-intermediate-flag",
            "test-user",
            [],
            ["email" => "unknown@example.com"],
            [],
            true
        );
        $unknownRootResult = $client->getFeatureFlag(
            "multivariate-root-flag",
            "test-user",
            [],
            ["email" => "unknown@example.com"],
            [],
            true
        );
        $this->assertEquals(false, $unknownLeafResult); // No matching email -> null variant -> false
        $this->assertEquals(false, $unknownIntermediateResult); // Dependency not satisfied
        $this->assertEquals(false, $unknownRootResult); // Chain broken
    }

    public function testMultiLevelMultivariateDependencyChain(): void
    {
        // Test multi-level multivariate dependency chain: dependent-flag -> intermediate-flag -> leaf-flag

        // Leaf flag: multivariate with "control" and "test" variants using person property overrides
        $leafFlag = [
            "id" => 1,
            "name" => "Leaf Flag",
            "key" => "leaf-flag",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            ["key" => "email", "operator" => "icontains", "value" => "@example.com", "type" => "person"]
                        ],
                        "rollout_percentage" => 100,
                        "variant" => "test",
                    ],
                    [
                        "properties" => [],
                        "rollout_percentage" => 100,
                        "variant" => "control",
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

        // Intermediate flag: depends on leaf flag being "control" variant
        $intermediateFlag = [
            "id" => 2,
            "name" => "Intermediate Flag",
            "key" => "intermediate-flag",
            "active" => true,
            "filters" => [
                "groups" => [
                    [
                        "properties" => [
                            [
                                "key" => "leaf-flag",
                                "operator" => "flag_evaluates_to",
                                "value" => "control",
                                "type" => "flag",
                                "dependency_chain" => ["leaf-flag"],
                            ],
                            ["key" => "variant_type", "operator" => "exact", "value" => "blue", "type" => "person"]
                        ],
                        "rollout_percentage" => 100,
                        "variant" => "blue",
                    ],
                    [
                        "properties" => [
                            [
                                "key" => "leaf-flag",
                                "operator" => "flag_evaluates_to",
                                "value" => "control",
                                "type" => "flag",
                                "dependency_chain" => ["leaf-flag"],
                            ],
                            ["key" => "variant_type", "operator" => "exact", "value" => "green", "type" => "person"]
                        ],
                        "rollout_percentage" => 100,
                        "variant" => "green",
                    ]
                ],
                "multivariate" => [
                    "variants" => [
                        ["key" => "blue", "name" => "Blue", "rollout_percentage" => 50],
                        ["key" => "green", "name" => "Green", "rollout_percentage" => 50]
                    ]
                ]
            ]
        ];

        $flagsByKey = [
            "leaf-flag" => $leafFlag,
            "intermediate-flag" => $intermediateFlag
        ];

        $evaluationCache = [];

        // Test 1: Leaf flag should evaluate to "control" when email condition is not satisfied
        $leafResult = FeatureFlag::matchFeatureFlagProperties(
            $leafFlag,
            "user-with-control-variant",
            ["email" => "test@other.com"], // This won't match @example.com condition
            [],
            $flagsByKey,
            $evaluationCache
        );
        // Since email doesn't match, it should fall back to the second condition which has variant "control"
        $this->assertEquals("control", $leafResult);

        // Test 2: Intermediate flag should evaluate to "blue" when dependency is satisfied and variant_type is "blue"
        $evaluationCache = []; // Reset cache
        $intermediateResult = FeatureFlag::matchFeatureFlagProperties(
            $intermediateFlag,
            "user-with-control-variant",
            // email doesn't match, so leaf-flag="control", variant_type="blue"
            ["email" => "test@other.com", "variant_type" => "blue"],
            [],
            $flagsByKey,
            $evaluationCache
        );
        $this->assertEquals("blue", $intermediateResult);

        // Test 3: Intermediate flag should evaluate to false when leaf dependency fails
        $evaluationCache = []; // Reset cache
        $intermediateResult = FeatureFlag::matchFeatureFlagProperties(
            $intermediateFlag,
            "user-with-test-variant",
            // This makes leaf-flag="test", breaking dependency
            ["email" => "test@example.com", "variant_type" => "blue"],
            [],
            $flagsByKey,
            $evaluationCache
        );
        $this->assertFalse($intermediateResult);
    }
}
