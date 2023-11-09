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

    public function user()
    {
      return $this->belongsTo(User::class);
    }

    public function sendMessage($conversationId, $body)
    {
      Message::create([
        'conversation_id' => $conversationId,
        'user_id' => $this->user->id,
        'body' => $body
      ]);
    }
}
