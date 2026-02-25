<?php

namespace WorkOS;

/**
 * Class WorkOS.
 *
 * This class allows users to get and set configuration for the package.
 */
class WorkOS
{
    /**
     * @var null|string WorkOS API key
     */
    private static $apiKey = null;

    /**
     * @var null|string WorkOS Client ID
     */
    private static $clientId = null;

    /**
     * @var string WorkOS base API URL.
     */
    private static $apiBaseUrl = "https://api.workos.com/";

    /**
     * @var string SDK identifier
     */
    private static $identifier = Version::SDK_IDENTIFIER;

    /**
     * @var string SDK version
     */
    private static $version = Version::SDK_VERSION;

    /**
     * @return null|string WorkOS API key
     */
    public static function getApiKey()
    {
        if (isset(self::$apiKey)) {
            return self::$apiKey;
        }

        $envValue = self::getEnvVariable("WORKOS_API_KEY");
        if ($envValue) {
            self::$apiKey = $envValue;
            return self::$apiKey;
        }

        $msg = "\$apiKey is required";
        throw new \WorkOS\Exception\ConfigurationException($msg);
    }

    /**
     * @param null|string $apiKey WorkOS API key
     */
    public static function setApiKey($apiKey)
    {
        self::$apiKey = $apiKey;
    }

    /**
     * @throws \WorkOS\Exception\ConfigurationException
     *
     * @return null|string WorkOS Client ID
     */
    public static function getClientId()
    {
        if (isset(self::$clientId)) {
            return self::$clientId;
        }

        $envValue = self::getEnvVariable("WORKOS_CLIENT_ID");
        if ($envValue) {
            self::$clientId = $envValue;
            return self::$clientId;
        }

        $msg = "\$clientId is required";
        throw new \WorkOS\Exception\ConfigurationException($msg);
    }

    /**
     * @param string $clientId WorkOS Client ID
     */
    public static function setClientId($clientId)
    {
        self::$clientId = $clientId;
    }

    /**
     * @return string WorkOS base API URL
     */
    public static function getApiBaseURL()
    {
        return self::$apiBaseUrl;
    }

    /**
     * @param string $apiBaseUrl WorkOS base API URL
     */
    public static function setApiBaseUrl($apiBaseUrl)
    {
        self::$apiBaseUrl = $apiBaseUrl;
    }

    /**
     * @param string $identifier SDK identifier
     */
    public static function setIdentifier($identifier)
    {
        self::$identifier = $identifier;
    }

    /**
     * @return string SDK identifier
     */
    public static function getIdentifier()
    {
        return self::$identifier;
    }

    /**
     * @param string $version SDK version
     */
    public static function setVersion($version)
    {
        self::$version = $version;
    }

    /**
     * @return string SDK version
     */
    public static function getVersion()
    {
        return self::$version;
    }

    /**
     * Get environment variable with fallback to cached config sources.
     * Checks in order: getenv(), $_ENV, $_SERVER
     *
     * @param string $key Environment variable name
     * @return string|false The environment variable value or false if not found
     */
    private static function getEnvVariable($key)
    {
        $value = getenv($key);
        if ($value !== false && $value !== '') {
            return $value;
        }

        if (isset($_ENV[$key]) && $_ENV[$key] !== '') {
            return $_ENV[$key];
        }

        if (isset($_SERVER[$key]) && $_SERVER[$key] !== '') {
            return $_SERVER[$key];
        }

        return false;
    }
}
