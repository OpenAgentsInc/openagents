<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Relations\BelongsToMany;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Flow extends Model
{
    use HasFactory;

    protected $guarded = [];

    public function agents(): BelongsToMany
    {
        return $this->belongsToMany(Agent::class, 'agent_flow');
    }

    public function nodes(): BelongsToMany
    {
        return $this->belongsToMany(Node::class);
    }
}
