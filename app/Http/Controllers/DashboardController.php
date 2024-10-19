<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use App\Models\Message;

class DashboardController extends Controller
{
    public function index()
    {
        $messages = [];
        if (auth()->check()) {
            $user = auth()->user();
            $messages = $user->messages()->orderBy('created_at', 'desc')->get();
        }

        return view('dashboard', compact('messages'));
    }
}