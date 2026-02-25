<?php

namespace PostHog\Test\Assets;

class MockedResponses
{
    public const FLAGS_REQUEST = [
        'config' => [
            'enable_collect_everything' => true,
        ],
        'editorParams' => [
        ],
        'isAuthenticated' => false,
        'supportedCompression' => [
            0 => 'gzip',
            1 => 'gzip-js',
            2 => 'lz64',
        ],
        'featureFlags' => [
            'simpleFlag' => true,
            'having_fun' => false,
            'enabled-flag' => true,
            'disabled-flag' => false,
            'multivariate-simple-test' => 'variant-simple-value',
            'simple-test' => true,
            'multivariate-test' => 'variant-value',
            'group-flag' => 'decide-fallback-value',
            'complex-flag' => 'decide-fallback-value',
            'beta-feature' => 'decide-fallback-value',
            'beta-feature2' => 'alakazam',
            'feature-1' => 'decide-fallback-value',
            'feature-2' => 'decide-fallback-value',
            'variant-1' => 'variant-1',
            'variant-3' => 'variant-3'
        ],
        'sessionRecording' => false,
    ];

    public const FLAGS_RESPONSE = [
        'featureFlags' => [
            'simpleFlag' => true,
            'having_fun' => false,
            'enabled-flag' => true,
            'disabled-flag' => false,
            'multivariate-simple-test' => 'variant-simple-value',
            'simple-test' => true,
            'multivariate-test' => 'variant-value',
            'group-flag' => 'decide-fallback-value',
            'complex-flag' => 'decide-fallback-value',
            'beta-feature' => 'decide-fallback-value',
            'beta-feature2' => 'alakazam',
            'feature-1' => 'decide-fallback-value',
            'feature-2' => 'decide-fallback-value',
            'variant-1' => 'variant-1',
            'variant-3' => 'variant-3',
            'json-payload' => true,
            'integer-payload' => true,
            'string-payload' => true,
            'array-payload' => true,
        ],
        'featureFlagPayloads' => [
            'simpleFlag' => '{"key":"simpleFlag"}',
            'having_fun' => '123',
            'enabled-flag' => '[0, 1, 2]',
            'multivariate-simple-test' => '"some string payload"',
            'json-payload' => '{"key":"value"}',
            'integer-payload' => '2500',
            'string-payload' => '"A String"',
            'array-payload' => '[1, 2, 3]',
        ],
    ];

    public const FLAGS_V2_RESPONSE = [
        'config' => [
            'enable_collect_everything' => true,
        ],
        'editorParams' => [
        ],
        'isAuthenticated' => false,
        'supportedCompression' => [
            0 => 'gzip',
            1 => 'gzip-js',
            2 => 'lz64',
        ],
        'flags' => [
            'simpleFlag' => [
                'key' => 'simpleFlag',
                'enabled' => true,
                'variant' => null,
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 1,
                    'payload' => '{"key":"simpleFlag"}',
                    'version' => 1,
                ]
            ],
            'having_fun' => [
                'key' => 'having_fun',
                'enabled' => false,
                'variant' => null,
                'reason' => [
                    'code' => 'no_condition_match',
                    'description' => 'No matching condition set',
                    'condition_index' => null
                ],
                'metadata' => [
                    'id' => 2,
                    'payload' => '123',
                    'version' => 1,
                ]
            ],
            'enabled-flag' => [
                'key' => 'enabled-flag',
                'enabled' => true,
                'variant' => null,
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 3',
                    'condition_index' => 2
                ],
                'metadata' => [
                    'id' => 3,
                    'payload' => '[0, 1, 2]',
                    'version' => 1,
                ]
            ],
            'disabled-flag' => [
                'key' => 'disabled-flag',
                'enabled' => false,
                'variant' => null,
                'reason' => [
                    'code' => 'no_condition_match',
                    'description' => 'No matching condition set',
                    'condition_index' => null
                ],
                'metadata' => [
                    'id' => 4,
                    'payload' => null,
                    'version' => 1,
                ]
            ],
            'multivariate-simple-test' => [
                'key' => 'multivariate-simple-test',
                'enabled' => true,
                'variant' => 'variant-simple-value',
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 5,
                    'payload' => '"some string payload"',
                    'version' => 1,
                ]
            ],
            'simple-test' => [
                'key' => 'simple-test',
                'enabled' => true,
                'variant' => null,
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 6,
                    'payload' => null,
                    'version' => 1,
                ]
            ],
            'multivariate-test' => [
                'key' => 'multivariate-test',
                'enabled' => true,
                'variant' => 'variant-value',
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 2',
                    'condition_index' => 1
                ],
                'metadata' => [
                    'id' => 7,
                    'payload' => null,
                    'version' => 3,
                ]
            ],
            'group-flag' => [
                'key' => 'group-flag',
                'enabled' => true,
                'variant' => 'decide-fallback-value',
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 8,
                    'payload' => null,
                    'version' => 1,
                ]
            ],
            'complex-flag' => [
                'key' => 'complex-flag',
                'enabled' => true,
                'variant' => 'decide-fallback-value',
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 9,
                    'payload' => null,
                    'version' => 1,
                ]
            ],
            'beta-feature' => [
                'key' => 'beta-feature',
                'enabled' => true,
                'variant' => 'decide-fallback-value',
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 10,
                    'payload' => null,
                    'version' => 1,
                ]
            ],
            'beta-feature2' => [
                'key' => 'beta-feature2',
                'enabled' => true,
                'variant' => 'alakazam',
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 11,
                    'payload' => null,
                    'version' => 1,
                ]
            ],
            'feature-1' => [
                'key' => 'feature-1',
                'enabled' => true,
                'variant' => 'decide-fallback-value',
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 12,
                    'payload' => null,
                    'version' => 1,
                ]
            ],
            'feature-2' => [
                'key' => 'feature-2',
                'enabled' => true,
                'variant' => 'decide-fallback-value',
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 13,
                    'payload' => null,
                    'version' => 1,
                ]
            ],
            'variant-1' => [
                'key' => 'variant-1',
                'enabled' => true,
                'variant' => 'variant-1',
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 14,
                    'payload' => null,
                    'version' => 1,
                ]
            ],
            'variant-3' => [
                'key' => 'variant-3',
                'enabled' => true,
                'variant' => 'variant-3',
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 15,
                    'payload' => null,
                    'version' => 1,
                ]
            ],
            'json-payload' => [
                'key' => 'json-payload',
                'enabled' => true,
                'variant' => null,
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 16,
                    'payload' => '{"key":"value"}',
                    'version' => 1,
                ]
            ],
            'integer-payload' => [
                'key' => 'integer-payload',
                'enabled' => true,
                'variant' => null,
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 17,
                    'payload' => '2500',
                    'version' => 1,
                ]
            ],
            'string-payload' => [
                'key' => 'string-payload',
                'enabled' => true,
                'variant' => null,
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 18,
                    'payload' => '"A String"',
                    'version' => 1,
                ]
            ],
            'array-payload' => [
                'key' => 'array-payload',
                'enabled' => true,
                'variant' => null,
                'reason' => [
                    'code' => 'condition_match',
                    'description' => 'Matched condition set 1',
                    'condition_index' => 0
                ],
                'metadata' => [
                    'id' => 19,
                    'payload' => '[1, 2, 3]',
                    'version' => 1,
                ]
            ],
        ],
        'sessionRecording' => false,
        'requestId' => '98487c8a-287a-4451-a085-299cd76228dd'
    ];

    public const LOCAL_EVALUATION_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "",
                "key" => "person-flag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "region",
                                    "value" => ["USA"],
                                    "operator" => "exact",
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 100
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => null
            ]
        ],
    ];

    public const LOCAL_EVALUATION_BOOLEAN_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "",
                "key" => "person-flag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "region_array",
                                    "value" => ["true"],
                                    "operator" => "exact",
                                    "type" => "person"
                                ],
                                [
                                    "key" => "region",
                                    "value" => "true",
                                    "operator" => "exact",
                                    "type" => "person"
                                ],
                            ],
                            "rollout_percentage" => 100
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => null
            ],
            [
                "id" => 2,
                "name" => "",
                "key" => "person-flag-with-boolean",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "region_array",
                                    "value" => [true],
                                    "operator" => "exact",
                                    "type" => "person"
                                ],
                                [
                                    "key" => "region",
                                    "value" => true,
                                    "operator" => "exact",
                                    "type" => "person"
                                ],
                            ],
                            "rollout_percentage" => 100
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => null
            ],
            [
                "id" => 2,
                "name" => "",
                "key" => "person-flag-with-boolean-icontains",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "region",
                                    "value" => true,
                                    "operator" => "icontains",
                                    "type" => "person"
                                ],
                            ],
                            "rollout_percentage" => 100
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => null
            ]
        ],
    ];

    public const LOCAL_EVALUATION_MULTIPLE_REQUEST = [
        'count' => 2,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "",
                "key" => "person-flag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "region",
                                    "value" => ["USA"],
                                    "operator" => "exact",
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 100
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => null
            ],
            [
                "id" => 2,
                "name" => "",
                "key" => "true-flag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                            ],
                            "rollout_percentage" => 100
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => null
            ]
        ],
    ];

    public const LOCAL_EVALUATION_WITH_NO_ROLLOUT_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "",
                "key" => "enabled-flag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                            ],
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => null
            ],
        ],
    ];

    public const LOCAL_EVALUATION_WITH_INACTIVE_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "",
                "key" => "enabled-flag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                            ],
                            "rollout_percentage" => 100
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => null
            ],
            [
                "id" => 1,
                "name" => "",
                "key" => "disabled-flag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                            ],
                            "rollout_percentage" => 100
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => false,
                "is_simple_flag" => true,
                "rollout_percentage" => null
            ]
        ],
    ];

    public const LOCAL_EVALUATION_WITH_COHORTS_REQUEST = [
        'flags' => [
            [
                'id' => 2,
                'name' => 'Beta Feature',
                'key' => 'beta-feature',
                'is_simple_flag' => false,
                'active' => true,
                'filters' => [
                    'groups' => [
                        [
                            'properties' => [
                                [
                                    'key' => 'region',
                                    'operator' => 'exact',
                                    'value' => ['USA'],
                                    'type' => 'person',
                                ],
                                [
                                    'key' => 'id',
                                    'value' => 98,
                                    'operator' => null,
                                    'type' => 'cohort',
                                ],
                            ],
                            'rollout_percentage' => 100,
                        ],
                    ],
                ],
            ]
        ],
        'cohorts' => [
            '98' => [
                'type' => 'OR',
                'values' => [
                    ['key' => 'id', 'value' => 1, 'type' => 'cohort'],
                    ['key' => 'nation', 'operator' => 'exact', 'value' => ['UK'], 'type' => 'person'],
                ],
            ],
            '1' => [
                'type' => 'AND',
                'values' => [
                    ['key' => 'other', 'operator' => 'exact', 'value' => ['thing'], 'type' => 'person'],
                ],
            ],
        ]

    ];

    public const LOCAL_EVALUATION_FOR_NEGATED_COHORTS_REQUEST = [
        'flags' => [
            [
                'id' => 2,
                'name' => 'Beta Feature',
                'key' => 'beta-feature',
                'is_simple_flag' => false,
                'active' => true,
                'filters' => [
                    'groups' => [
                        [
                            'properties' => [
                                [
                                    'key' => 'region',
                                    'operator' => 'exact',
                                    'value' => ['USA'],
                                    'type' => 'person',
                                ],
                                [
                                    'key' => 'id',
                                    'value' => 98,
                                    'operator' => null,
                                    'type' => 'cohort',
                                ],
                            ],
                            'rollout_percentage' => 100,
                        ],
                    ],
                ],
            ]
        ],
        'cohorts' => [
            '98' => [
                'type' => 'OR',
                'values' => [
                    ['key' => 'id', 'value' => 1, 'type' => 'cohort'],
                    ['key' => 'nation', 'operator' => 'exact', 'value' => ['UK'], 'type' => 'person'],
                ],
            ],
            '1' => [
                'type' => 'AND',
                'values' => [
                    ['key' => 'other', 'operator' => 'exact', 'value' => ['thing'], 'type' => 'person', 'negation' => true],
                ],
            ],
        ],
    ];

    public const LOCAL_EVALUATION_GROUP_PROPERTIES_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "group flag",
                "key" => "group-flag",
                "filters" => [
                    "aggregation_group_type_index" => 0,
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "group_type_index" => 0,
                                    "key" => "name",
                                    "value" => ["Project Name 1"],
                                    "operator" => "exact",
                                    "type" => "group"
                                ]
                            ],
                            "rollout_percentage" => 35
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => null
            ]
        ],
        'group_type_mapping' => [
            "0" => "company",
            "1" => "project"
        ]
    ];

    public const LOCAL_EVALUATION_COMPLEX_FLAG_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "",
                "key" => "complex-flag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "region",
                                    "value" => ["USA"],
                                    "operator" => "exact",
                                    "type" => "person"
                                ],
                                [
                                    "key" => "name",
                                    "value" => ["Aloha"],
                                    "operator" => "exact",
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 100
                        ],
                        [
                            "properties" => [
                                [
                                    "key" => "email",
                                    "value" => ["a@b.com"],
                                    "operator" => "exact",
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 35
                        ],
                        [
                            "properties" => [
                                [
                                    "key" => "doesnt_matter",
                                    "value" => ["1", "2"],
                                    "operator" => "exact",
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 0
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => null
            ]
        ]
    ];

    public const LOCAL_EVALUATION_VARIANT_OVERRIDES_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "Beta feature",
                "key" => "beta-feature",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "email",
                                    "value" => "test@posthog.com",
                                    "operator" => "exact",
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 100,
                            "variant" => "second-variant"
                        ],
                        [
                            "rollout_percentage" => 50,
                            "variant" => "first-variant"
                        ]
                    ],
                    "multivariate" => [
                        "variants" => [
                            [
                                "key" => "first-variant",
                                "name" => "First Variant",
                                "rollout_percentage" => 50
                            ],
                            [
                                "key" => "second-variant",
                                "name" => "Second Variant",
                                "rollout_percentage" => 25
                            ],
                            [
                                "key" => "third-variant",
                                "name" => "Third Variant",
                                "rollout_percentage" => 25
                            ]
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => false,
                "rollout_percentage" => null
            ]
        ],
    ];

    public const LOCAL_EVALUATION_CLASHING_VARIANT_OVERRIDES_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "Beta feature",
                "key" => "beta-feature",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "email",
                                    "value" => "test@posthog.com",
                                    "operator" => "exact",
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 100,
                            "variant" => "second-variant"
                        ],
                        [
                            "properties" => [
                                [
                                    "key" => "email",
                                    "value" => "test@posthog.com",
                                    "operator" => "exact",
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 100,
                            "variant" => "first-variant"
                        ],
                        [
                            "rollout_percentage" => 50,
                            "variant" => "first-variant"
                        ]
                    ],
                    "multivariate" => [
                        "variants" => [
                            [
                                "key" => "first-variant",
                                "name" => "First Variant",
                                "rollout_percentage" => 50
                            ],
                            [
                                "key" => "second-variant",
                                "name" => "Second Variant",
                                "rollout_percentage" => 25
                            ],
                            [
                                "key" => "third-variant",
                                "name" => "Third Variant",
                                "rollout_percentage" => 25
                            ]
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => false,
                "rollout_percentage" => null
            ]
        ],
    ];

    public const LOCAL_EVALUATION_INVALID_VARIANT_OVERRIDES_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "Beta feature",
                "key" => "beta-feature",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "email",
                                    "value" => "test@posthog.com",
                                    "operator" => "exact",
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 100,
                            "variant" => "second???"
                        ],
                        [
                            "rollout_percentage" => 50,
                            "variant" => "first???"
                        ]
                    ],
                    "multivariate" => [
                        "variants" => [
                            [
                                "key" => "first-variant",
                                "name" => "First Variant",
                                "rollout_percentage" => 50
                            ],
                            [
                                "key" => "second-variant",
                                "name" => "Second Variant",
                                "rollout_percentage" => 25
                            ],
                            [
                                "key" => "third-variant",
                                "name" => "Third Variant",
                                "rollout_percentage" => 25
                            ]
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => false,
                "rollout_percentage" => null
            ]
        ],
    ];

    public const LOCAL_EVALUATION_MULTIPLE_VARIANT_OVERRIDES_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "Beta feature",
                "key" => "beta-feature",
                "filters" => [
                    "groups" => [
                        [
                            "rollout_percentage" => 100,
                        ],
                        [
                            "properties" => [
                                [
                                    "key" => "email",
                                    "value" => "test@posthog.com",
                                    "operator" => "exact",
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 100,
                            "variant" => "second-variant"
                        ],
                        [
                            "rollout_percentage" => 50,
                            "variant" => "third-variant"
                        ]
                    ],
                    "multivariate" => [
                        "variants" => [
                            [
                                "key" => "first-variant",
                                "name" => "First Variant",
                                "rollout_percentage" => 50
                            ],
                            [
                                "key" => "second-variant",
                                "name" => "Second Variant",
                                "rollout_percentage" => 25
                            ],
                            [
                                "key" => "third-variant",
                                "name" => "Third Variant",
                                "rollout_percentage" => 25
                            ]
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => false,
                "rollout_percentage" => null
            ]
        ],
    ];

    public const LOCAL_EVALUATION_CONDITIONS_ORDER_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "Test Flag",
                "key" => "test-flag",
                "active" => true,
                "deleted" => false,
                "filters" => [
                    "groups" => [
                        // First condition: 100% rollout for everyone
                        [
                            "rollout_percentage" => 100,
                        ],
                        // Second condition: VIP users get a specific variant
                        // This used to be evaluated first due to sorting, but now it's evaluated second
                        [
                            "properties" => [
                                [
                                    "key" => "email",
                                    "value" => "@vip.com",
                                    "operator" => "icontains",
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 100,
                            "variant" => "vip-variant"
                        ],
                    ],
                    "multivariate" => [
                        "variants" => [
                            [
                                "key" => "control",
                                "name" => "Control",
                                "rollout_percentage" => 50
                            ],
                            [
                                "key" => "test",
                                "name" => "Test",
                                "rollout_percentage" => 50
                            ],
                            [
                                "key" => "vip-variant",
                                "name" => "VIP Variant",
                                "rollout_percentage" => 0
                            ]
                        ]
                    ]
                ],
            ]
        ],
    ];


    public const EXPERIENCE_CONITNUITY_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "Beta Feature",
                "key" => "beta-feature",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [],
                            "rollout_percentage" => 100
                        ]
                    ]
                ],
                "ensure_experience_continuity" => true,
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => 100
            ]
        ],
    ];

    public const FALLBACK_TO_FLAGS_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "feature 1",
                "key" => "feature-1",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "id",
                                    "value" => 98,
                                    "operator" => null,
                                    "type" => "cohort"
                                ]
                            ],
                            "rollout_percentage" => 100
                        ],
                    ],
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => false,
                "rollout_percentage" => null

            ],
            [
                "id" => 2,
                "name" => "feature 2",
                "key" => "feature-2",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "region",
                                    "value" => ["USA"],
                                    "operator" => null,
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 100
                        ],
                    ],
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => false,
                "rollout_percentage" => null

            ]
        ]
    ];

    public const LOCAL_EVALUATION_SIMPLE_EMPTY_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "",
                "key" => "simple-flag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [],
                            "rollout_percentage" => 0
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
            ]
        ],
    ];

    public const LOCAL_EVALUATION_SIMPLE_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "",
                "key" => "simple-flag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [],
                            "rollout_percentage" => 100
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => 100
            ]
        ],
    ];

    public const SIMPLE_PARTIAL_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "",
                "key" => "simple-flag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [],
                            "rollout_percentage" => 45
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => 45
            ]
        ],
    ];

    public const MULTIVARIATE_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "",
                "key" => "multivariate-flag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [],
                            "rollout_percentage" => 55,
                        ],
                    ],
                    "multivariate" => [
                        "variants" => [
                            [
                                "key" => "first-variant",
                                "name" => "First Variant",
                                "rollout_percentage" => 50
                            ],
                            [
                                "key" => "second-variant",
                                "name" => "Second Variant",
                                "rollout_percentage" => 20
                            ],
                            [
                                "key" => "third-variant",
                                "name" => "Third Variant",
                                "rollout_percentage" => 20
                            ],
                            [
                                "key" => "fourth-variant",
                                "name" => "Fourth Variant",
                                "rollout_percentage" => 5
                            ],
                            [
                                "key" => "fifth-variant",
                                "name" => "Fifth Variant",
                                "rollout_percentage" => 5
                            ]
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => 45
            ]
        ],
    ];

    public const MULTIPLE_FLAGS_LOCAL_EVALUATE_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "",
                "key" => "variant-1",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [],
                            "rollout_percentage" => 100
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => false,
                "rollout_percentage" => 100
            ],
            [
                "id" => 2,
                "name" => "",
                "key" => "variant-2",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [],
                            "rollout_percentage" => 0
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => false,
                "rollout_percentage" => 100
            ]
        ],
    ];

    public const MULTIPLE_FLAGS_REQUEST = [
        'count' => 1,
        'next' => null,
        'previous' => null,
        'flags' => [
            [
                "id" => 1,
                "name" => "",
                "key" => "variant-1",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [],
                            "rollout_percentage" => 100
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => false,
                "rollout_percentage" => 100
            ],
            [
                "id" => 2,
                "name" => "",
                "key" => "variant-2",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [],
                            "rollout_percentage" => 0
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => false,
                "rollout_percentage" => 100
            ],
            [
                "id" => 3,
                "name" => "",
                "key" => "variant-3",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [
                                [
                                    "key" => "country",
                                    "value" => ["USA"],
                                    "operator" => null,
                                    "type" => "person"
                                ]
                            ],
                            "rollout_percentage" => 0
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => false,
                "rollout_percentage" => 100
            ]
        ],
    ];

    public const SIMPLE_FLAG_EXAMPLE_REQUEST = [
        "count" => 1,
        "next" => null,
        "previous" => null,
        "results" => [
            [
                "id" => 719,
                "name" => "",
                "key" => "simpleFlag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [],
                            "rollout_percentage" => null
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => true,
                "rollout_percentage" => null
            ],
            [
                "id" => 720,
                "name" => "",
                "key" => "enabled-flag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [],
                            "rollout_percentage" => null
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => false,
                "rollout_percentage" => null
            ],
            [
                "id" => 721,
                "name" => "",
                "key" => "disabled-flag",
                "filters" => [
                    "groups" => [
                        [
                            "properties" => [],
                            "rollout_percentage" => null
                        ]
                    ]
                ],
                "deleted" => false,
                "active" => true,
                "is_simple_flag" => false,
                "rollout_percentage" => null
            ],
        ]
    ];

    public const LOCAL_EVALUATION_WITH_STATIC_COHORT = [
        'flags' => [
            [
                'id' => 1,
                'key' => 'multi-condition-flag',
                'filters' => [
                    'groups' => [
                        [
                            'properties' => [
                                [
                                    'key' => 'id',
                                    'value' => 999,
                                    'type' => 'cohort'
                                ]
                            ],
                            'rollout_percentage' => 100,
                            'variant' => 'set-1'
                        ],
                        [
                            'properties' => [
                                [
                                    'key' => '$geoip_country_code',
                                    'operator' => 'exact',
                                    'value' => ['DE'],
                                    'type' => 'person'
                                ]
                            ],
                            'rollout_percentage' => 100,
                            'variant' => 'set-8'
                        ]
                    ],
                    'multivariate' => [
                        'variants' => [
                            ['key' => 'set-1', 'rollout_percentage' => 50],
                            ['key' => 'set-8', 'rollout_percentage' => 50]
                        ]
                    ],
                    'payloads' => [
                        'set-1' => '{"message": "local-payload-1"}',
                        'set-8' => '{"message": "local-payload-8"}'
                    ]
                ],
                'active' => true,
                'is_simple_flag' => false
            ]
        ],
        'cohorts' => []
    ];

    public const FLAGS_WITH_STATIC_COHORT_RESPONSE = [
        'featureFlags' => [
            'multi-condition-flag' => 'set-1'
        ],
        'featureFlagPayloads' => [
            'multi-condition-flag' => '{"message": "from-api"}'
        ]
    ];

    public const LOCAL_EVALUATION_WITH_STATIC_COHORT_FOR_PAYLOAD = [
        'flags' => [
            [
                'id' => 2,
                'key' => 'flag-with-payload',
                'filters' => [
                    'groups' => [
                        [
                            'properties' => [
                                [
                                    'key' => 'id',
                                    'value' => 999,
                                    'type' => 'cohort'
                                ]
                            ],
                            'rollout_percentage' => 100
                        ]
                    ],
                    'payloads' => [
                        'true' => '{"message": "local-payload"}'
                    ]
                ],
                'active' => true,
                'is_simple_flag' => false
            ]
        ],
        'cohorts' => []
    ];

    public const FLAGS_WITH_STATIC_COHORT_PAYLOAD_RESPONSE = [
        'featureFlags' => [
            'flag-with-payload' => true
        ],
        'featureFlagPayloads' => [
            'flag-with-payload' => '{"message": "from-api"}'
        ]
    ];
}
