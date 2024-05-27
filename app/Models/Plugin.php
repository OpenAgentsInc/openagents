<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Plugin extends Model
{
    use HasFactory;

    protected $fillable = [
        'kind',
        'name',
        'description',
        'tos',
        'privacy',
        'web',
        'picture',
        'tags',
        'mini_template',
        'input_template',
        'output_template',
        'user_id',
        'author'
    ];

    protected $casts = [
        'tags' => 'json',
        'mini_template' => 'json',
        'sockets' => 'json',
    ];


    public function user()
    {
        return $this->belongsTo(User::class);
    }
}
