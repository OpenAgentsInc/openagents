<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

class PoolJobPaymentRequest extends Model
{
    use HasFactory;

    protected $fillable = [
        'pool_job_id',
        'amount',
        'protocol',
        'currency',
        'target',
        'paid',
    ];

    public function poolJob(): BelongsTo
    {
        return $this->belongsTo(PoolJob::class);
    }
}
