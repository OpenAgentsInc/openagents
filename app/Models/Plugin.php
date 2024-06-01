<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Factories\HasFactory;
use Illuminate\Database\Eloquent\Model;

class Plugin extends Model
{
    use HasFactory;

    protected $fillable = [
        'name',
        'description',
        'tos',
        'privacy',
        'web',
        'picture',
        'tags',
        'input_sockets',
        'output_sockets',
        'input_template',
        'secrets',
        'file_link',
        'user_id',
        'author',
        'payment',
        'wasm_upload',
    ];

    protected $casts = [
        'tags' => 'json',
        'secrets' => 'json',
        'wasm_upload' => 'json',

    ];

    public function user()
    {
        return $this->belongsTo(User::class);
    }
}
