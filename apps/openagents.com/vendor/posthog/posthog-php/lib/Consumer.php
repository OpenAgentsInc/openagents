<?php

namespace PostHog;

abstract class Consumer
{
    protected $type = "Consumer";

    protected $options;
    protected $apiKey;

    /**
     * Store our apiKey and options as part of this consumer
     * @param string $apiKey
     * @param array $options
     */
    public function __construct($apiKey, $options = array())
    {
        $this->apiKey = $apiKey;
        $this->options = $options;
    }

    /**
     * Captures a user action
     *
     * @param array $message
     * @return boolean whether the capture call succeeded
     */
    abstract public function capture(array $message);

    /**
     * Tags properties about the user.
     *
     * @param array $message
     * @return boolean whether the identify call succeeded
     */
    abstract public function identify(array $message);

    /**
     * Aliases from one user id to another
     *
     * @param array $message
     * @return boolean whether the alias call succeeded
     */
    abstract public function alias(array $message);

    /**
     * Check whether debug mode is enabled
     * @return boolean
     */
    protected function debug()
    {
        return isset($this->options["debug"]) ? $this->options["debug"] : false;
    }

    /**
     * Check whether we should connect to the API using SSL. This is enabled by
     * default with connections which make batching requests. For connections
     * which can save on round-trip times, you may disable it.
     * @return boolean
     */
    protected function ssl()
    {
        return isset($this->options["ssl"]) ? $this->options["ssl"] : true;
    }

    /**
     * On an error, try and call the error handler, if debugging output to
     * error_log as well.
     * @param string $code
     * @param string $msg
     */
    protected function handleError($code, $msg)
    {
        if (isset($this->options['error_handler'])) {
            $handler = $this->options['error_handler'];
            $handler($code, $msg);
        }

        if ($this->debug()) {
            error_log("[PostHog][" . $this->type . "] " . $msg);
        }
    }
}
