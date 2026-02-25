<?php

namespace PostHog;

use Exception;

class InconclusiveMatchException extends Exception
{
    public function errorMessage()
    {
        $errorMsg = 'Error on line ' . $this->getLine() . ' in ' . $this->getFile() . ': <b> Inconclusive Match:' . $this->getMessage() . '</b>'; //phpcs:ignore
        return $errorMsg;
    }
}
