<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsToMany;

class Flow extends Model
{
    use HasFactory;

    protected $fillable = ['name'];

    public function agents(): BelongsToMany
    {
        return $this->belongsToMany(Agent::class);
    }

    public function nodes(): BelongsToMany
    {
        return $this->belongsToMany(Node::class);
    }
}
