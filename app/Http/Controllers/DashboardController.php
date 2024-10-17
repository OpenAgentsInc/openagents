<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use App\Models\Message;

class DashboardController extends Controller
{
    public function index()
    {
        $user = auth()->user();
        $messages = $user ? $user->messages()->orderBy('created_at', 'desc')->get() : collect();

        return view('dashboard', compact('messages'));
    }
}