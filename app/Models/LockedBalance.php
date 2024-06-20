<?php

namespace App\Models;

use App\Enums\Currency;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class LockedBalance extends Model
{
    use HasFactory;

    protected $guarded = [];

    protected $casts = [
        'currency' => Currency::class,
    ];
}
