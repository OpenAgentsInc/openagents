<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Brain extends Model
{
    use HasFactory;

    public function datapoints()
    {
        return $this->hasMany(Datapoint::class);
    }
}
