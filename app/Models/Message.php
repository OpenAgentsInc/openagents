<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class Message extends Model
{
    use HasFactory;

    protected $fillable = ['body', 'user_id', 'agent_id', 'session_id', 'model', 'input_tokens', 'output_tokens', 'agent_id'];

    public function agent(): BelongsTo
    {
        return $this->belongsTo(Agent::class);
    }
}
