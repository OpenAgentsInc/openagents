import React from 'react'
import AuthLayout from '../Layouts/AuthLayout'

function LoginPage () {
  return (
    <div className="h-full flex flex-col items-center justify-center" style={{ background: 'linear-gradient(rgb(12, 13, 14) 0%, rgb(8, 8, 8) 50%)' }}>
      <div className="mb-8 gap-y-6 w-full flex flex-col items-center justify-center">
        <img className="w-20 h-20" src="/images/sqlogo-t.png" />
        <h1 className="text-xl font-bold">Log in to OpenAgents</h1>
        <a href="/login/x">
        <button className="inline-flex items-center px-4 py-2 bg-black border border-gray-300 dark:border-gray-500 rounded-md font-semibold text-gray-700 dark:text-gray-300 tracking-wide shadow-sm hover:bg-gray-50 dark:hover:bg-gray-700 focus:outline-none disabled:opacity-25 transition ease-in-out duration-150 w-full flex justify-center gap-2 mb-0 h-[44px]">
          <div className="h-5 w-5 rounded-full bg-white" />
          <span className="text-md">Continue with X</span>
        </button>
        </a>
      </div>
    </div>
  );
}

LoginPage.layout = page => <AuthLayout children={page} title="Log in to OpenAgents" />

export default LoginPage
