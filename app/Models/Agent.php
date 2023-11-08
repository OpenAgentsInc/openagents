<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Agent extends Model
{
    use HasFactory;

    protected $guarded = [];

    public function conversations()
    {
      return $this->hasMany(Conversation::class);
    }

    public function tasks()
    {
      return $this->hasMany(Task::class);
    }
}
