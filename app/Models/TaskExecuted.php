<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class TaskExecuted extends Model
{
    use HasFactory;

    protected $guarded = [];

    // belongs to a Task
    public function task(): BelongsTo
    {
        return $this->belongsTo(Task::class);
    }

    public function conversation(): BelongsTo
    {
        return $this->belongsTo(Conversation::class);
    }
}
