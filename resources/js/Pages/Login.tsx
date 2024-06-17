import React from "react";
import AuthLayout from "../Layouts/AuthLayout";

function LoginPage() {
  return (
    <div
      className="h-full w-full fixed flex flex-col items-center justify-center"
      style={{
        background: "linear-gradient(rgb(12, 13, 14) 0%, rgb(8, 8, 8) 50%)",
      }}
    >
      <div className="mb-8 gap-y-6 w-full flex flex-col items-center justify-center">
        <img className="w-20 h-20" src="/images/sqlogo-t.png" />
        <h1 className="text-xl font-bold">Log in to OpenAgents</h1>

        <div className="mt-2 w-[350px]">
          <a href="/login/x">
            <button className="inline-flex items-center justify-center text-[16px] w-full h-[48px] border border-white rounded-md gap-2">
              <div className="h-5 w-5 rounded-full bg-white" />
              <span>Continue with X</span>
            </button>
          </a>
        </div>
      </div>
    </div>
  );
}

LoginPage.layout = (page) => (
  <AuthLayout children={page} title="Log in to OpenAgents" />
);

export default LoginPage;
