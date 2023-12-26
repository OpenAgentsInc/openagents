<?php

namespace App\Models;

use App\Events\StepCreated;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Step extends Model
{
    use HasFactory;

    protected $guarded = [];

    public function agent()
    {
        return $this->belongsTo(Agent::class);
    }

    public function task()
    {
        return $this->belongsTo(Task::class);
    }
}
