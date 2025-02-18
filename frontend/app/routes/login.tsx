import type { Route } from "../+types/root";

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Login" },
    { name: "description", content: "Login to OpenAgents" },
  ];
}

export default function Login() {
  return (
    <div className="flex justify-center mx-2 md:mx-6">
      <div className="w-[60rem] max-w-full px-4 py-6 border border-white">
        <div className="max-w-md mx-auto">
          <h1 className="text-lg font-bold mb-6">Login</h1>

          <form className="space-y-4">
            <div>
              <label htmlFor="email" className="block text-sm mb-1">
                Email
              </label>
              <input
                type="email"
                id="email"
                className="w-full px-3 py-2 bg-black border border-white text-white text-sm focus:outline-none focus:ring-1 focus:ring-white"
                required
              />
            </div>

            <div>
              <label htmlFor="password" className="block text-sm mb-1">
                Password
              </label>
              <input
                type="password"
                id="password"
                className="w-full px-3 py-2 bg-black border border-white text-white text-sm focus:outline-none focus:ring-1 focus:ring-white"
                required
              />
            </div>

            <button
              type="submit"
              className="w-full bg-black hover:bg-zinc-900 text-white text-sm px-4 py-2 border border-white shadow-nav hover:shadow-nav-hover active:shadow-nav-active transition-all duration-nav ease-nav"
            >
              Login
            </button>
          </form>
        </div>
      </div>
    </div>
  );
}
