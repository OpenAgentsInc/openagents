<?php

namespace App\AI;

use App\Models\Agent;
use Illuminate\Database\Eloquent\Collection;
use LaravelIdea\Helper\App\Models\_IH_Agent_C;

class Agents
{
    public static function AGENTS(): _IH_Agent_C|Collection|array
    {
        return Agent::all();
    }
}
