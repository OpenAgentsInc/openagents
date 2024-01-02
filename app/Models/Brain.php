<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Brain extends Model
{
    use HasFactory;

    protected $guarded = [];

    public function datapoints()
    {
        return $this->hasMany(Datapoint::class);
    }

    public function createDatapoint(string $point)
    {
        return $this->datapoints()->create([
            'data' => $point
        ]);
    }
}
