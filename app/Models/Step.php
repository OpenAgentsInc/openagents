<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Step extends Model
{
  use HasFactory;

  protected $guarded = [];

  public function agent()
  {
    return $this->belongsTo(Agent::class);
  }

  public function artifacts()
  {
    return $this->hasMany(Artifact::class);
  }

  public function run()
  {
    return $this->belongsTo(Run::class);
  }

  public function task()
  {
    return $this->belongsTo(Task::class);
  }
}
