<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Memory extends Model
{
    use HasFactory;

    // ALL THE ATTRIBUTES WE WANT TO MASS ASSIGN MUST BE SPECIFIED IN THE `$FILLABLE` PROPERTY
    protected $fillable = ['description', 'last_accessed'];
}
