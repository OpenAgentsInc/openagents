<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Message extends Model
{
    /** @use HasFactory<\Database\Factories\MessageFactory> */
    use HasFactory;

    protected $guarded = [];

    protected $with = ['toolInvocations'];

    protected $appends = ['toolInvocations'];

    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class);
    }

    public function thread(): BelongsTo
    {
        return $this->belongsTo(Thread::class);
    }

    public function toolInvocations(): HasMany
    {
        return $this->hasMany(ToolInvocation::class);
    }

    public function getToolInvocationsAttribute()
    {
        return $this->toolInvocations()->get();
    }
}