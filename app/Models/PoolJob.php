<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

class PoolJob extends Model
{
    use HasFactory;

    protected $fillable = ['payload', 'status', 'job_id', 'thread_id', 'content', 'agent_id', 'warmup'];

    public function agent(): BelongsTo
    {
        return $this->belongsTo(Agent::class);
    }

    public function paymentRequests(): HasMany
    {
        return $this->hasMany(PoolJobPaymentRequest::class);
    }
}
