<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class ToolInvocation extends Model
{
    use HasFactory;

    protected $fillable = [
        'message_id',
        'tool_name',
        'input',
        'output',
        'status',
    ];

    protected $casts = [
        'input' => 'array',
        'output' => 'array',
    ];

    public function message()
    {
        return $this->belongsTo(Message::class);
    }
}
