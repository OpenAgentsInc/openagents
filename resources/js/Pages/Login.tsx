import React, { useState, useCallback } from "react";
import { Icon, TextField } from "@shopify/polaris";
import { EmailIcon, LogoGoogleIcon, LogoXIcon } from "@shopify/polaris-icons";
import NostrIcon from "./nostr.svg";
import AuthLayout from "../Layouts/AuthLayout";

function LoginPage() {
  const [value, setValue] = useState("");

  const handleChange = useCallback((e) => setValue(e.target.value), []);
  return (
    <div
      className="h-full w-full fixed flex flex-col items-center justify-center"
      style={{
        background: "linear-gradient(rgb(12, 13, 14) 0%, rgb(8, 8, 8) 50%)",
      }}
    >
      <div className="-mt-4 mb-8 gap-y-6 w-full flex flex-col items-center justify-center">
        <img className="w-24 h-24" src="/images/sqlogo-t.png" />
        <h1 className="text-2xl font-bold">Log in to OpenAgents</h1>

        <div className="flex flex-col gap-y-5 mt-4 w-[350px]">
          <form>
            <label for="email" class="pl-1">
              Email
            </label>
            <input
              id="email"
              type="email"
              autoFocus
              label="Email"
              value={value}
              onChange={handleChange}
              autoComplete="off"
              className="mt-1 flex h-[48px] w-full rounded-md border-2 bg-transparent p-3 pr-10 text-[16px] placeholder:text-[#777A81] focus-visible:outline-none focus-visible:ring-0 focus:visible:border-white focus-visible:ring-white"
            />
            <button
              type="submit"
              className="mt-3 inline-flex items-center justify-center text-[16px] w-full h-[48px] border border-white rounded-md gap-2"
            >
              <div>
                <Icon source={EmailIcon} className="h-5 w-5" />
              </div>
              <span>Continue with email</span>
            </button>
          </form>

          <p class="text-center">or</p>

          <div class="flex flex-row gap-x-2">
            <button className="w-full h-[48px] border border-white rounded-md">
              <Icon source={LogoGoogleIcon} className="h-5 w-5" />
            </button>
            <button className="w-full h-[48px] border border-white rounded-md">
              <Icon source={LogoXIcon} className="h-5 w-5" />
            </button>
            <button className="flex justify-center items-center w-full h-[48px] border border-white rounded-md">
              <img src={NostrIcon} className="h-5 w-5" alt="Nostr" />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

LoginPage.layout = (page) => (
  <AuthLayout children={page} title="Log in to OpenAgents" />
);

export default LoginPage;
