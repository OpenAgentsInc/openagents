<?php

// phpcs:disable PSR1.Files.SideEffects
namespace PostHog;

// The $errorMessages array captures logged messages.
$errorMessages = [];

function error_log($message)
{
    global $errorMessages;
    $errorMessages[] = $message;
}
