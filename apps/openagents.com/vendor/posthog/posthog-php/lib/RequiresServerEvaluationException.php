<?php

namespace PostHog;

use Exception;

class RequiresServerEvaluationException extends Exception
{
    public function errorMessage()
    {
        $errorMsg = 'Error on line ' . $this->getLine() . ' in ' . $this->getFile() . ': <b> Requires Server Evaluation:' . $this->getMessage() . '</b>'; //phpcs:ignore
        return $errorMsg;
    }
}
