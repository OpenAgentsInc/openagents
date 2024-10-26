<?php

namespace App\Http\Controllers;

use App\Models\Inquiry;
use Illuminate\Http\Request;
use Inertia\Inertia;

class InquireController extends Controller
{
    public function page()
    {
        return Inertia::render('Inquire');
    }

    public function submit(Request $request)
    {
        $validated = $request->validate([
            'email' => ['required', 'email'],
            'comment' => ['required', 'string', 'min:10'],
        ]);

        Inquiry::create($validated);

        return redirect()->back()->with('success', 'Thank you for your inquiry. We will get back to you soon.');
    }
}