<?php

namespace App\Http\Controllers\Htmx;

use App\Http\Controllers\Controller;

class ChatController extends Controller
{
    public function index()
    {
        return view('htmx.chat');
    }

    public function store()
    {
        $message = request('message-input');

        return "hi: $message";
    }
}
