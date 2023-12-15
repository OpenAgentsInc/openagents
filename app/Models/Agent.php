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

    public function steps()
    {
        return $this->hasMany(Step::class);
    }

    public function tasks()
    {
        return $this->hasMany(Task::class);
    }

    public function thoughts()
    {
        return $this->hasMany(Thought::class);
    }

    public function user()
    {
        return $this->belongsTo(User::class);
    }

    public function sendMessage($conversationId, $body)
    {
        Message::create([
            'sender' => 'agent',
            'conversation_id' => $conversationId,
            'user_id' => $this->user->id,
            'body' => $body
        ]);
    }

    public function reflect()
    {
        // Grab all the steps for this agent
        $steps = $this->steps()->get();

        Thought::create([
            'agent_id' => $this->id,
            'body' => "I notice that I have {$steps->count()} steps."
        ]);
    }
}
