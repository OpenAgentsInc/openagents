import { Link, Outlet, useLocation } from "react-router"
import { Button } from "~/components/ui/button"

const navItems = [
  { to: "/", text: "Home" },
  { to: "/onyx", text: "Mobile App" },
  { to: "/video-series", text: "Video Series" },
  { to: "/services", text: "Services" },
  { to: "/company", text: "Company" },
  { to: "/coming-soon", text: "Coming Soon" },
];

export default function Layout() {
  const location = useLocation();

  return (
    <div className="dark flex justify-center mx-2 md:mx-6">
      <div className="w-[60rem] max-w-full my-6 px-4 py-6 border border-white">
        <div className="grid grid-cols-3 lg:grid-cols-6 gap-4 mb-6">
          {navItems.map(({ to, text }) => (
            <Link key={to} to={to} className="contents">
              <Button
                variant="nav"
                size="sm"
                showDot={location.pathname === to}
                className="w-full"
              >
                {text}
              </Button>
            </Link>
          ))}
        </div>
        <Outlet />
      </div>
    </div>
  );
}
