<?php

namespace App\Services;

use App\Contracts\AIGateway;

class OpenAIGateway implements AIGateway
{
    public function inference($input)
    { /* Implementation */
    }

    public function embedding($input)
    { /* Implementation */
    }
    // OpenAI specific methods
}

class GPUtopiaGateway implements AIGateway
{
    public function inference($input)
    { /* Implementation */
    }

    public function embedding($input)
    { /* Implementation */
    }
    // GPUtopia specific methods
}

// Other gateways...
