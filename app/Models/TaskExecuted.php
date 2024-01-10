<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class TaskExecuted extends Model
{
    use HasFactory;

    protected $guarded = [];

    // belongs to a Task
    public function task()
    {
        return $this->belongsTo(Task::class);
    }

    public function conversation()
    {
        return $this->belongsTo(Conversation::class);
    }
}
