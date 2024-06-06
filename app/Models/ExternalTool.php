<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class ExternalTool extends Model
{
    use HasFactory;

    /**
     * The attributes that are mass assignable.
     *
     * @var array
     */
    protected $fillable = [
        'agent_id',
        'external_uid',
    ];

    /**
     * Get the agent that owns the external tool.
     */
    public function agent()
    {
        return $this->belongsTo(Agent::class);
    }
}
