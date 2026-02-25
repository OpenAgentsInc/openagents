<?php

namespace PostHog\Consumer;

use Exception;
use PostHog\Consumer;

class File extends Consumer
{
    protected $type = "File";

    private $file_handle;

    /**
     * The file consumer writes capture and identify calls to a file.
     * @param string $apiKey
     * @param array $options
     *     string "filename" - where to log the posthog calls
     */
    public function __construct($apiKey, $options = array())
    {
        if (!isset($options["filename"])) {
            $options["filename"] = sys_get_temp_dir() . DIRECTORY_SEPARATOR . "posthog.log";
        }
        parent::__construct($apiKey, $options);

        try {
            $this->file_handle = fopen($options["filename"], "a");
            chmod($options["filename"], 0777);
        } catch (Exception $e) {
            $this->handleError($e->getCode(), $e->getMessage());
        }
    }

    public function __destruct()
    {
        if ($this->file_handle && "Unknown" != get_resource_type($this->file_handle)) {
            fclose($this->file_handle);
        }
    }

    /**
     * Define getter method for consumer type
     *
     * @return string
     */
    public function getConsumer()
    {
        return $this->type;
    }

    /**
     * Captures a user action
     *
     * @param array $message
     * @return bool whether the capture call succeeded
     */
    public function capture(array $message)
    {
        return $this->write($message);
    }

    /**
     * Tags properties about the user.
     *
     * @param array $message
     * @return bool whether the identify call succeeded
     */
    public function identify(array $message)
    {
        return $this->write($message);
    }

    /**
     * Aliases from one user id to another
     *
     * @param array $message
     * @return boolean whether the alias call succeeded
     */
    public function alias(array $message)
    {
        return $this->write($message);
    }

    /**
     * Writes the API call to a file as line-delimited json
     * @param array $body post body content.
     * @return bool whether the request succeeded
     */
    private function write($body)
    {
        if (!$this->file_handle) {
            return false;
        }

        $content = json_encode($body);
        $content .= "\n";

        return fwrite($this->file_handle, $content) == strlen($content);
    }
}
