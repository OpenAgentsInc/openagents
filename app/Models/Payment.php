<?php

namespace App\Models;

use App\Enums\Currency;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\SoftDeletes;

class Payment extends Model
{
    use HasFactory, SoftDeletes;

    protected $guarded = [];

    protected $casts = [
        'currency' => Currency::class,
    ];

    // Define relationships
    public function sources()
    {
        return $this->morphedByMany(User::class, 'source', 'payment_sources')
            ->withPivot('created_at', 'updated_at')
            ->using(PaymentSource::class)
            ->morphedByMany(Agent::class, 'source', 'payment_sources')
            ->withPivot('created_at', 'updated_at')
            ->using(PaymentSource::class);
    }

    public function destinations()
    {
        return $this->morphedByMany(User::class, 'destination', 'payment_destinations')
            ->withPivot('created_at', 'updated_at')
            ->using(PaymentDestination::class)
            ->morphedByMany(Agent::class, 'destination', 'payment_destinations')
            ->withPivot('created_at', 'updated_at')
            ->using(PaymentDestination::class);
    }
}
