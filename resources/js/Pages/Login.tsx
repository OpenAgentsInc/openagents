import React from 'react'
import AuthLayout from '../Layouts/AuthLayout'

function LoginPage () {
  return (
    <div className="h-full flex flex-col items-center justify-center" style={{ background: 'linear-gradient(rgb(12, 13, 14) 0%, rgb(8, 8, 8) 50%)' }}>
      <div className="mb-8 gap-y-6 w-full flex flex-col items-center justify-center">
        <img className="w-20 h-20" src="/images/sqlogo-t.png" />
        <h1 className="text-xl font-bold">Log in to OpenAgents</h1>
      </div>
    </div>
  );
}

LoginPage.layout = page => <AuthLayout children={page} title="Log in to OpenAgents" />

export default LoginPage
