<?php

namespace App\Http\Controllers;

use Inertia\Inertia;

class ChatController
{
    public function chat()
    {
        return Inertia::render('Chat');
    }
}
