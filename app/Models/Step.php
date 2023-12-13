<?php

namespace App\Models;

use App\Events\StepCreated;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Step extends Model
{
    use HasFactory;

    protected $guarded = [];

    // // Every time a Step is created, broadcast to its run channel
    // protected static function booted()
    // {
    //     static::created(function ($step) {
    //         \Log::info('StepCreated...');
    //         StepCreated::dispatch($step);
    //         // broadcast(new StepCreated($step));
    //     });
    // }

    public function agent()
    {
        return $this->belongsTo(Agent::class);
    }

    public function artifacts()
    {
        return $this->hasMany(Artifact::class);
    }

    public function run()
    {
        return $this->belongsTo(Run::class);
    }

    public function task()
    {
        return $this->belongsTo(Task::class);
    }
}
