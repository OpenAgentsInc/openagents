<?php

namespace App\Models;

use App\Events\PaymentCreated;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Payment extends Model
{
    use HasFactory;

    protected $guarded = [];

    protected static function booted()
    {
        static::created(function ($payment) {
            broadcast(new PaymentCreated($payment));
        });
    }
}
