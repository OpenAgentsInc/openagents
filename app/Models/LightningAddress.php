<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class LightningAddress extends Model
{
    use HasFactory;

    protected $fillable = [
        'address',
        'user_id',
        'vanity',
    ];

    /**
     * Get the user that owns the lightning address.
     */
    public function user()
    {
        return $this->belongsTo(User::class);
    }
}
