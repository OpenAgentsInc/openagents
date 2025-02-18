import { Link, Outlet } from "react-router";

const navItems = [
  { to: "/", text: "Home" },
  { to: "/onyx", text: "Mobile App" },
  { to: "/video-series", text: "Video Series" },
  { to: "/services", text: "Services" },
  { to: "/company", text: "Company" },
  { to: "/coming-soon", text: "Coming Soon" },
];

export default function Layout() {
  return (
    <div className="flex justify-center mx-2 md:mx-6">
      <div className="w-[60rem] max-w-full my-6 px-4 py-6 border border-white">
        <div className="grid grid-cols-3 lg:grid-cols-6 gap-4 mb-6">
          {navItems.map(({ to, text }) => (
            <Link
              key={to}
              to={to}
              className="relative bg-black hover:bg-zinc-900 text-white text-xs inline-flex items-center justify-center gap-2 whitespace-nowrap select-none text-center align-middle no-underline outline-none px-3 md:px-4 py-2 md:py-1 border border-white shadow-nav hover:shadow-nav-hover active:shadow-nav-active transition-all duration-nav ease-nav group touch-manipulation"
            >
              <span className="transition-transform duration-nav ease-nav">
                {text}
              </span>
            </Link>
          ))}
        </div>
        <Outlet />
      </div>
    </div>
  );
}
