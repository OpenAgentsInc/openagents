<?php

namespace App\Services;

use App\Models\Log;

class LocalLogger
{
    public function log($object)
    {
        $log = [
            'data' => json_encode($object),
        ];

        Log::create($log);
    }
}
