<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\BelongsToMany;
use Illuminate\Database\Eloquent\SoftDeletes;

class Agent extends Model
{
    use HasFactory, SoftDeletes;

    protected $guarded = [];

    //    public function files(): BelongsToMany
    //    {
    //        return $this->belongsToMany(File::class);
    //    }

    public function flows(): BelongsToMany
    {
        return $this->belongsToMany(Flow::class);
    }

    public function threads(): BelongsToMany
    {
        return $this->belongsToMany(Thread::class);
    }

    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class);
    }
}
