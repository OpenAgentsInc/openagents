<?php

namespace App\Models;

use App\Events\StepCreated;
use App\Traits\StepActions;
use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Step extends Model
{
    use HasFactory, StepActions;

    protected $guarded = [];

    public function run()
    {
        // Based on the category, run the appropriate StepAction. [validation, embedding, similarity_search, inference]
        return $this->$category();
    }

    public function agent()
    {
        return $this->belongsTo(Agent::class);
    }

    public function task()
    {
        return $this->belongsTo(Task::class);
    }
}
