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

    public function sources()
    {
        return $this->hasMany(PaymentSource::class);
    }

    public function destinations()
    {
        return $this->hasMany(PaymentDestination::class);
    }
}
