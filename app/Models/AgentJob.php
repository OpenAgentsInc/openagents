<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class AgentJob extends Model
{
    use HasFactory;

    protected $fillable = ['is_rag_ready', 'agent_id', 'job_id'];
}
