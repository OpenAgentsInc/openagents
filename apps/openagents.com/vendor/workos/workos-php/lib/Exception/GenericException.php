<?php

namespace WorkOS\Exception;

/**
 * Class GenericException
 *
 * Generic WorkOS Exception.
 */
class GenericException extends \Exception implements WorkOSException
{
    public $data;

    /**
     * GenericException constructor.
     *
     * @param string $message Exception message
     * @param null|array $data Blob
     */
    public function __construct($message, ?array $data = null)
    {
        $this->message = $message;

        if (!empty($data)) {
            $this->data = $data;
        } else {
            $this->data = array();
        }
    }
}
