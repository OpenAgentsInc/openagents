<?php

namespace App\Http\Controllers;

use App\Models\Payment;

class ExplorerController extends Controller
{
    public function index()
    {
        $recentPayments = Payment::latest()->take(50)->get();

        return view('explorer.explorer', compact('recentPayments'));
    }
}
