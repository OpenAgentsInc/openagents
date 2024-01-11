<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;
use Pgvector\Laravel\Vector;

class Thought extends Model
{
    use HasFactory;

    protected $guarded = [];

    protected $casts = [
        'embedding' => Vector::class,
    ];

    public function agent()
    {
        return $this->belongsTo(Agent::class);
    }

    public function brain()
    {
        return $this->belongsTo(Brain::class);
    }
}
