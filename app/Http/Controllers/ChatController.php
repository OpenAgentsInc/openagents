<?php

namespace App\Http\Controllers;

// An experimental HTMX refactor of our Livewire Chat.php component

class ChatController extends Controller
{
    public function index()
    {
        return view('chat');
    }
}
