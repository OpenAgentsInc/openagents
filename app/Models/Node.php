<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Node extends Model
{
    use HasFactory;

    protected $guarded = [];

    // Each node can have multiple ports.
    public function ports(): HasMany
    {
        return $this->hasMany(Port::class);
    }
}
