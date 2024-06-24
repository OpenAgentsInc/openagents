<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Payin extends Model
{
    use HasFactory;

    protected $dates = ['last_check'];

    protected $guarded = [];

    public function user()
    {
        return $this->belongsTo(User::class);
    }
}
