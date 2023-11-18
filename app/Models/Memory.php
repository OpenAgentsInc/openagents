<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Memory extends Model
{
    use HasFactory;

    // All the attributes we want to mass assign must be specified in the `$fillable` property
    protected $fillable = ['description', 'last_accessed'];
}
