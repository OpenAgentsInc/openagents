<?php

namespace App\Services;

use App\Models\Log;

class LocalLogger
{
    public function log($message)
    {
        $log = [
            'data' => json_encode([
                'message' => $message,
            ]),
        ];

        Log::create($log);
    }
}
