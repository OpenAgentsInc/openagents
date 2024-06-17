<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Thread extends Model
{
    use HasFactory;

    protected $fillable = [
        'model',
        'session_id',
        'user_id',
        'agent_id',
        'title',
    ];

    protected $guarded = [
        'session_id',
    ];

    public function messages(): HasMany
    {
        return $this->hasMany(Message::class);
    }

    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class);
    }

    public function agent(): BelongsTo
    {
        return $this->belongsTo(Agent::class);
    }

    public function scopeWhereSessionId($query, $sessionId)
    {
        return $query->where('session_id', $sessionId);
    }

    public function getModelAttribute($value)
    {
        $model = $value;
        if (! $model && $this->messages->count() > 0) {
            $lastMessage = $this->messages->last();
            $model = $lastMessage->model;
        }

        return $model;
    }
}
