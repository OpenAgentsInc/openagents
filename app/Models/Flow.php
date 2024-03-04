<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Flow extends Model
{
    use HasFactory;

    protected $guarded = [];

    public function nodes()
    {
        return $this->belongsToMany(Node::class);
    }
}
