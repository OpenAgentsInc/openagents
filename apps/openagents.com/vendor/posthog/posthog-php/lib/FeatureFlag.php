<?php

namespace PostHog;

const LONG_SCALE = 0xfffffffffffffff;

class FeatureFlag
{
    public static function matchProperty($property, $propertyValues)
    {
        $key = $property["key"];
        $operator = $property["operator"] ?? "exact";
        $value = $property["value"];

        if (!array_key_exists($key, $propertyValues)) {
            throw new InconclusiveMatchException("Can't match properties without a given property value");
        }

        if ($operator == "is_not_set") {
            throw new InconclusiveMatchException("can't match properties with operator is_not_set");
        }

        $overrideValue = $propertyValues[$key];

        if ($operator == "exact") {
            return FeatureFlag::computeExactMatch($value, $overrideValue);
        }

        if ($operator == "is_not") {
            return !FeatureFlag::computeExactMatch($value, $overrideValue);
        }

        if ($operator == "is_set") {
            return array_key_exists($key, $propertyValues);
        }

        if ($operator == "icontains") {
            return strpos(strtolower(FeatureFlag::valueToString($overrideValue)), strtolower(FeatureFlag::valueToString($value))) !== false;
        }

        if ($operator == "not_icontains") {
            return strpos(strtolower(FeatureFlag::valueToString($overrideValue)), strtolower(FeatureFlag::valueToString($value))) == false;
        }

        if (in_array($operator, ["regex", "not_regex"])) {
            $regexValue = FeatureFlag::prepareValueForRegex($value);
            if (FeatureFlag::isRegularExpression($regexValue)) {
                if ($overrideValue === null) {
                    return false;
                }
                $returnValue = preg_match($regexValue, $overrideValue) ? true : false;
                if ($operator == "regex") {
                    return $returnValue;
                } else {
                    return !$returnValue;
                }
            } else {
                return false;
            }
        }

        if (in_array($operator, ["gt", "gte", "lt", "lte"])) {
            $parsedValue = null;

            if (is_numeric($value)) {
                $parsedValue = floatval($value);
            }

            if (!is_null($parsedValue) && !is_null($overrideValue)) {
                if (is_string($overrideValue)) {
                    return FeatureFlag::compare($overrideValue, FeatureFlag::valueToString($value), $operator);
                } else {
                    return FeatureFlag::compare($overrideValue, $parsedValue, $operator, "numeric");
                }
            } else {
                return FeatureFlag::compare(FeatureFlag::valueToString($overrideValue), FeatureFlag::valueToString($value), $operator);
            }
        }

        if (in_array($operator, ["is_date_before", "is_date_after"])) {
            $parsedDate = FeatureFlag::relativeDateParseForFeatureFlagMatching($value);

            if (is_null($parsedDate)) {
                $parsedDate = FeatureFlag::convertToDateTime($value);
            }

            if (is_null($parsedDate)) {
                throw new InconclusiveMatchException("The date set on the flag is not a valid format");
            }

            $overrideDate = FeatureFlag::convertToDateTime($overrideValue);
            if ($operator == 'is_date_before') {
                return $overrideDate < $parsedDate;
            } else {
                return $overrideDate > $parsedDate;
            }
        }

        return false;
    }

    public static function matchCohort($property, $propertyValues, $cohortProperties, $flagsByKey = null, $evaluationCache = null, $distinctId = null)
    {
        $cohortId = strval($property["value"]);
        if (!array_key_exists($cohortId, $cohortProperties)) {
            throw new RequiresServerEvaluationException(
                "cohort {$cohortId} not found in local cohorts - " .
                "likely a static cohort that requires server evaluation"
            );
        }

        $propertyGroup = $cohortProperties[$cohortId];
        return FeatureFlag::matchPropertyGroup($propertyGroup, $propertyValues, $cohortProperties, $flagsByKey, $evaluationCache, $distinctId);
    }

    public static function matchPropertyGroup($propertyGroup, $propertyValues, $cohortProperties, $flagsByKey = null, $evaluationCache = null, $distinctId = null)
    {
        if (!$propertyGroup) {
            return true;
        }

        $propertyGroupType = $propertyGroup["type"];
        $properties = $propertyGroup["values"];

        if (!$properties || count($properties) === 0) {
            // empty groups are no-ops, always match
            return true;
        }

        $errorMatchingLocally = false;

        if (array_key_exists("values", $properties[0])) {
            // a nested property group
            foreach ($properties as $prop) {
                try {
                    $matches = FeatureFlag::matchPropertyGroup($prop, $propertyValues, $cohortProperties, $flagsByKey, $evaluationCache, $distinctId);
                    if ($propertyGroupType === 'AND') {
                        if (!$matches) {
                            return false;
                        }
                    } else {
                        // OR group
                        if ($matches) {
                            return true;
                        }
                    }
                } catch (RequiresServerEvaluationException $err) {
                    // Immediately propagate - this condition requires server-side data
                    throw $err;
                } catch (InconclusiveMatchException $err) {
                    $errorMatchingLocally = true;
                }
            }

            if ($errorMatchingLocally) {
                throw new InconclusiveMatchException("Can't match cohort without a given cohort property value");
            }
            // if we get here, all matched in AND case, or none matched in OR case
            return $propertyGroupType === 'AND';
        } else {
            foreach ($properties as $prop) {
                try {
                    $matches = false;
                    $propType = $prop["type"] ?? null;
                    if ($propType === 'cohort') {
                        $matches = FeatureFlag::matchCohort($prop, $propertyValues, $cohortProperties, $flagsByKey, $evaluationCache, $distinctId);
                    } elseif ($propType === 'flag') {
                        $matches = FeatureFlag::evaluateFlagDependency($prop, $flagsByKey, $evaluationCache, $distinctId, $propertyValues, $cohortProperties);
                    } else {
                        $matches = FeatureFlag::matchProperty($prop, $propertyValues);
                    }

                    $negation = $prop["negation"] ?? false;

                    if ($propertyGroupType === 'AND') {
                        // if negated property, do the inverse
                        if (!$matches && !$negation) {
                            return false;
                        }
                        if ($matches && $negation) {
                            return false;
                        }
                    } else {
                        // OR group
                        if ($matches && !$negation) {
                            return true;
                        }
                        if (!$matches && $negation) {
                            return true;
                        }
                    }
                } catch (RequiresServerEvaluationException $err) {
                    // Immediately propagate - this condition requires server-side data
                    throw $err;
                } catch (InconclusiveMatchException $err) {
                    // If this is a flag dependency error, preserve the original message
                    if ($propType === 'flag') {
                        throw $err;
                    }
                    $errorMatchingLocally = true;
                }
            }

            if ($errorMatchingLocally) {
                throw new InconclusiveMatchException("can't match cohort without a given cohort property value");
            }

            // if we get here, all matched in AND case, or none matched in OR case
            return $propertyGroupType === 'AND';
        }
    }

    public static function relativeDateParseForFeatureFlagMatching($value)
    {
        $regex = "/^-?(?<number>[0-9]+)(?<interval>[a-z])$/";
        $parsedDt = new \DateTime("now", new \DateTimeZone("UTC"));
        if (preg_match($regex, $value, $matches)) {
            $number = intval($matches["number"]);

            if ($number >= 10_000) {
                // Guard against overflow, disallow numbers greater than 10_000
                return null;
            }

            $interval = $matches["interval"];
            if ($interval == "h") {
                $parsedDt->sub(new \DateInterval("PT{$number}H"));
            } elseif ($interval == "d") {
                $parsedDt->sub(new \DateInterval("P{$number}D"));
            } elseif ($interval == "w") {
                $parsedDt->sub(new \DateInterval("P{$number}W"));
            } elseif ($interval == "m") {
                $parsedDt->sub(new \DateInterval("P{$number}M"));
            } elseif ($interval == "y") {
                $parsedDt->sub(new \DateInterval("P{$number}Y"));
            } else {
                return null;
            }

            return $parsedDt;
        } else {
            return null;
        }
    }

    private static function convertToDateTime($value)
    {
        if ($value instanceof \DateTime) {
            return $value;
        } elseif (is_string($value)) {
            try {
                $date = new \DateTime($value);
                if (!is_nan($date->getTimestamp())) {
                    return $date;
                }
            } catch (Exception $e) {
                throw new InconclusiveMatchException("{$value} is in an invalid date format");
            }
        } else {
            throw new InconclusiveMatchException("The date provided {$value} must be a string or date object");
        }
    }

    private static function computeExactMatch($value, $overrideValue)
    {
        if (is_array($value)) {
            return in_array(strtolower(FeatureFlag::valueToString($overrideValue)), array_map('strtolower', array_map(fn($val) => FeatureFlag::valueToString($val), $value)));
        }
        return strtolower(FeatureFlag::valueToString($value)) == strtolower(FeatureFlag::valueToString($overrideValue));
    }

    private static function valueToString($value)
    {
        if (is_bool($value)) {
            return $value ? "true" : "false";
        } else {
            return strval($value);
        }
    }

    private static function compare($lhs, $rhs, $operator, $type = "string")
    {
        // If type is string, we use strcmp to compare the two strings
        // If type is numeric, we use <=> to compare the two numbers

        if ($type == "string") {
            $comparison = strcmp($lhs, $rhs);
        } else {
            $comparison = $lhs <=> $rhs;
        }

        if ($operator == "gt") {
            return $comparison > 0;
        } elseif ($operator == "gte") {
            return $comparison >= 0;
        } elseif ($operator == "lt") {
            return $comparison < 0;
        } elseif ($operator == "lte") {
            return $comparison <= 0;
        }

        throw new \Exception("Invalid operator: " . $operator);
    }

    private static function hash($key, $distinctId, $salt = "")
    {
        $hashKey = sprintf("%s.%s%s", $key, $distinctId, $salt);
        $hashVal = base_convert(substr(sha1($hashKey), 0, 15), 16, 10);

        return $hashVal / LONG_SCALE;
    }

    private static function getMatchingVariant($flag, $distinctId)
    {
        $variants = FeatureFlag::variantLookupTable($flag);

        foreach ($variants as $variant) {
            if (
                FeatureFlag::hash($flag["key"], $distinctId, "variant") >= $variant["value_min"]
                && FeatureFlag::hash($flag["key"], $distinctId, "variant") < $variant["value_max"]
            ) {
                return $variant["key"];
            }
        }

        return null;
    }

    private static function variantLookupTable($featureFlag)
    {
        $lookupTable = [];
        $valueMin = 0;
        $multivariates = (($featureFlag['filters'] ?? [])['multivariate'] ?? [])['variants'] ?? [];

        foreach ($multivariates as $variant) {
            $valueMax = $valueMin + $variant["rollout_percentage"] / 100;

            array_push($lookupTable, [
                "value_min" => $valueMin,
                "value_max" => $valueMax,
                "key" => $variant["key"]
            ]);
            $valueMin = $valueMax;
        }

        return $lookupTable;
    }

    public static function matchFeatureFlagProperties($flag, $distinctId, $properties, $cohorts = [], $flagsByKey = null, $evaluationCache = null)
    {
        $flagConditions = ($flag["filters"] ?? [])["groups"] ?? [];
        $isInconclusive = false;

        foreach ($flagConditions as $condition) {
            try {
                if (FeatureFlag::isConditionMatch($flag, $distinctId, $condition, $properties, $cohorts, $flagsByKey, $evaluationCache)) {
                    $variantOverride = $condition["variant"] ?? null;
                    $flagVariants = (($flag["filters"] ?? [])["multivariate"] ?? [])["variants"] ?? [];
                    $variantKeys = array_map(function ($variant) {
                        return $variant["key"];
                    }, $flagVariants);

                    if ($variantOverride && in_array($variantOverride, $variantKeys)) {
                        return $variantOverride;
                    } else {
                        return FeatureFlag::getMatchingVariant($flag, $distinctId) ?? true;
                    }
                }
            } catch (RequiresServerEvaluationException $e) {
                // Immediately propagate - this condition requires server-side data
                throw $e;
            } catch (InconclusiveMatchException $e) {
                // If this is a flag dependency error, preserve the original message
                if (
                    strpos($e->getMessage(), "Cannot evaluate flag dependency") !== false ||
                    strpos($e->getMessage(), "Circular dependency detected") !== false
                ) {
                    throw $e;
                }
                $isInconclusive = true;
            }
        }

        if ($isInconclusive) {
            throw new InconclusiveMatchException("Can't determine if feature flag is enabled or not with given properties"); //phpcs:ignore
        }

        return false;
    }

    private static function isConditionMatch($featureFlag, $distinctId, $condition, $properties, $cohorts, $flagsByKey = null, $evaluationCache = null)
    {
        $rolloutPercentage = array_key_exists("rollout_percentage", $condition) ? $condition["rollout_percentage"] : null;

        if (count($condition['properties'] ?? []) > 0) {
            foreach ($condition['properties'] as $property) {
                $matches = false;
                $propertyType = $property['type'] ?? null;
                if ($propertyType == 'cohort') {
                    $matches = FeatureFlag::matchCohort($property, $properties, $cohorts, $flagsByKey, $evaluationCache, $distinctId);
                } elseif ($propertyType == 'flag') {
                    $matches = FeatureFlag::evaluateFlagDependency($property, $flagsByKey, $evaluationCache, $distinctId, $properties, $cohorts);
                } else {
                    $matches = FeatureFlag::matchProperty($property, $properties);
                }

                if (!$matches) {
                    return false;
                }
            }

            if (is_null($rolloutPercentage)) {
                return true;
            }
        }

        if (!is_null($rolloutPercentage) && FeatureFlag::hash($featureFlag["key"], $distinctId) > ($rolloutPercentage / 100)) { //phpcs:ignore
            return false;
        }

        return true;
    }

    private static function isRegularExpression($string)
    {
        if ($string === null) {
            return false;
        }
        set_error_handler(function () {
        }, E_WARNING);
        $isRegularExpression = preg_match($string, "") !== false;
        restore_error_handler();
        return $isRegularExpression;
    }

    private static function prepareValueForRegex($value)
    {
        $regex = $value;

        // If delimiter already exists, do nothing
        if (FeatureFlag::isRegularExpression($regex)) {
            return $regex;
        }

        if (substr($regex, 0, 1) != "/") {
            $regex = "/" . $regex;
        }

        if (substr($regex, -1) != "/") {
            $regex = $regex . "/";
        }

        return $regex;
    }

    public static function evaluateFlagDependency($property, $flagsByKey, $evaluationCache, $distinctId, $properties, $cohortProperties)
    {
        if ($flagsByKey === null || $evaluationCache === null) {
            throw new InconclusiveMatchException(sprintf(
                "Cannot evaluate flag dependency on '%s' without flags_by_key and evaluation_cache",
                $property["key"] ?? "unknown"
            ));
        }

        // Check if dependency_chain is present - it should always be provided for flag dependencies
        if (!array_key_exists("dependency_chain", $property)) {
            throw new InconclusiveMatchException(sprintf(
                "Cannot evaluate flag dependency on '%s' without dependency_chain",
                $property["key"] ?? "unknown"
            ));
        }

        $dependencyChain = $property["dependency_chain"];

        // Handle circular dependency (empty chain means circular)
        if (count($dependencyChain) === 0) {
            throw new InconclusiveMatchException(sprintf(
                "Circular dependency detected for flag '%s'",
                $property["key"] ?? "unknown"
            ));
        }

        // The flag key to evaluate is in the "key" field
        $depFlagKey = $property["key"] ?? null;
        if (!$depFlagKey) {
            throw new InconclusiveMatchException(sprintf(
                "Flag dependency missing 'key' field: %s",
                json_encode($property)
            ));
        }

        // Check if we've already evaluated this flag
        if (!array_key_exists($depFlagKey, $evaluationCache)) {
            // Need to evaluate this dependency first
            $depFlag = $flagsByKey[$depFlagKey] ?? null;
            if (!$depFlag) {
                // Missing flag dependency - cannot evaluate locally
                $evaluationCache[$depFlagKey] = null;
                throw new InconclusiveMatchException(sprintf(
                    "Cannot evaluate flag dependency '%s' - flag not found in local flags",
                    $depFlagKey
                ));
            } else {
                // Check if the flag is active (same check as in Client::computeFlagLocally)
                if (!($depFlag["active"] ?? false)) {
                    $evaluationCache[$depFlagKey] = false;
                } else {
                    // Recursively evaluate the dependency
                    try {
                        $depResult = FeatureFlag::matchFeatureFlagProperties(
                            $depFlag,
                            $distinctId,
                            $properties,
                            $cohortProperties,
                            $flagsByKey,
                            $evaluationCache
                        );
                        $evaluationCache[$depFlagKey] = $depResult;
                    } catch (InconclusiveMatchException $e) {
                        // If we can't evaluate a dependency, store null and propagate the error
                        $evaluationCache[$depFlagKey] = null;
                        throw new InconclusiveMatchException(sprintf(
                            "Cannot evaluate flag dependency '%s': %s",
                            $depFlagKey,
                            $e->getMessage()
                        ));
                    }
                }
            }
        }

        // Get the evaluated flag value
        $flagValue = $evaluationCache[$depFlagKey];
        if ($flagValue === null) {
            // Previously inconclusive - raise error again
            throw new InconclusiveMatchException(sprintf(
                "Flag dependency '%s' was previously inconclusive",
                $depFlagKey
            ));
        }

        // Now check if the flag value matches the expected value in the property
        $expectedValue = $property["value"] ?? null;
        $operator = $property["operator"] ?? "exact";

        if ($expectedValue !== null) {
            // For flag dependencies, we need to compare the actual flag result with expected value
            // using the flag_evaluates_to operator logic
            if ($operator === "flag_evaluates_to") {
                return FeatureFlag::matchesDependencyValue($expectedValue, $flagValue);
            } else {
                // This should never happen, but just to be defensive
                throw new InconclusiveMatchException(sprintf(
                    "Flag dependency property for '%s' has invalid operator '%s'",
                    $depFlagKey,
                    $operator
                ));
            }
        }

        // If no value check needed, return true (all dependencies passed)
        return true;
    }

    public static function matchesDependencyValue($expectedValue, $actualValue)
    {
        // String variant case - check for exact match or boolean true
        if (is_string($actualValue) && strlen($actualValue) > 0) {
            if (is_bool($expectedValue)) {
                // Any variant matches boolean true
                return $expectedValue;
            } elseif (is_string($expectedValue)) {
                // variants are case-sensitive, hence our comparison is too
                return $actualValue === $expectedValue;
            } else {
                return false;
            }
        }
        // Boolean case - must match expected boolean value
        elseif (is_bool($actualValue) && is_bool($expectedValue)) {
            return $actualValue === $expectedValue;
        }

        // Default case
        return false;
    }
}
