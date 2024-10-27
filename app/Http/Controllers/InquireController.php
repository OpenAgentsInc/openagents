<?php

namespace App\Http\Controllers;

use App\Models\Inquiry;
use Illuminate\Http\Request;
use Inertia\Inertia;

class InquireController extends Controller
{
    public function page()
    {
        return Inertia::render('Inquire', [
            'success' => session('success'),
        ]);
    }

    public function submit(Request $request)
    {
        $validated = $request->validate([
            'inquiry_type' => ['required', 'string', 'in:general_question,request_demo,custom_agents,bulk_credits,other'],
            'email' => ['required', 'email'],
            'comment' => ['required', 'string', 'min:10'],
        ]);

        Inquiry::create($validated);

        return redirect()->route('inquire')->with('success', 'Thank you for your inquiry. We will get back to you soon.');
    }
}