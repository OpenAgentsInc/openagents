<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class StepExecuted extends Model
{
    use HasFactory;

    protected $guarded = [];

    public function step()
    {
        return $this->belongsTo(Step::class);
    }

    public function task_executed()
    {
        return $this->belongsTo(TaskExecuted::class);
    }

    public function user()
    {
        return $this->belongsTo(User::class);
    }
}
