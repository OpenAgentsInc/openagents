<?php

namespace App\Http\Controllers;

use Illuminate\Http\Request;
use Illuminate\Support\Facades\View;
use Mauricius\LaravelHtmx\Http\HtmxResponse;

class ProfileController extends Controller
{
    public function index()
    {
        return view('profile');
    }

    public function update(Request $request)
    {
        // Validate the request data
        $validatedData = $request->validate([
            'name' => 'required|max:255',
            'email' => 'required|email|max:255',
            'password' => 'nullable|min:8|confirmed',
        ]);

        // Get the authenticated user
        $user = auth()->user();

        // Update user's profile
        $user->name = $validatedData['name'];
        $user->email = $validatedData['email'];
        if (!empty($validatedData['password'])) {
            $user->password = bcrypt($validatedData['password']);
        }
        $user->save();

        // session()->flash('success', 'Profile updated successfully.');

        // Redirect back to the profile page with a success message
        // return redirect()->route('profile')->with('success', 'Profile updated successfully.');
        // return View::renderFragment('profile', 'edit-form');
        // return with(new HtmxResponse())
        //     ->renderFragment('profile', 'edit-form')
        //     ->addTrigger('displaySuccessMessage');
        // Define the success message
        $successMessage = 'Profile updated successfully.';

        // Render only the 'edit-form' fragment of the 'profile' view, including the success message
        return with(new HtmxResponse())
            ->renderFragment('profile', 'edit-form', compact('successMessage'));
    }
}
