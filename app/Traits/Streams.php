<?php

namespace App\Traits;

use App\Services\EventManager;
use App\Services\LocalLogger;

trait Streams
{
    public function stream($name, $content, $replace = false)
    {
        $manager = EventManager::getInstance();
        $this->logEvent($name, $content);
        $manager->streamEvent($name, $content, $replace);
    }

    private function logEvent($name, $content)
    {
        $logger = new LocalLogger();
        $logger->log([
            'event' => $name,
            'content' => $content,
        ]);
    }

    protected function ensureStreamResponseStarted()
    {
        $manager = EventManager::getInstance();
        $manager->ensureStreamResponseStarted();
    }
}
