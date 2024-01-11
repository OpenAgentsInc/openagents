<?php

namespace App\Models;

use App\Services\QueenbeeGateway;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class File extends Model
{
    use HasFactory;

    protected $guarded = [];

    // belongs to a user
    public function user()
    {
        return $this->belongsTo(User::class);
    }
}
