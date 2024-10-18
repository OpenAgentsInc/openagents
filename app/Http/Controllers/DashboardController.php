<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use App\Models\Message;

class DashboardController extends Controller
{
    public function index()
    {
        if (!auth()->check()) {
            return view('home');
        }

        $user = auth()->user();
        $messages = $user->messages()->orderBy('created_at', 'desc')->get();

        return view('dashboard', compact('messages'));
    }
}