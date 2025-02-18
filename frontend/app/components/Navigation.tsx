import { NavLink } from 'react-router-dom';

const navItems = [
  { href: '/', text: 'Home' },
  { href: '/onyx', text: 'Mobile App' },
  { href: '/video-series', text: 'Video Series' },
  { href: '/services', text: 'Services' },
  { href: '/company', text: 'Company' },
  { href: '/coming-soon', text: 'Coming Soon' },
];

export default function Navigation() {
  return (
    <div className="grid grid-cols-3 lg:grid-cols-6 gap-4 mb-6">
      {navItems.map(({ href, text }) => (
        <NavLink
          key={href}
          to={href}
          className={({ isActive }) =>
            `relative bg-black hover:bg-zinc-900 text-white text-xs inline-flex items-center justify-center gap-2 whitespace-nowrap select-none text-center align-middle no-underline outline-none px-3 md:px-4 py-2 md:py-1 border ${
              isActive ? 'border-white/90' : 'border-white'
            } shadow-nav hover:shadow-nav-hover active:shadow-nav-active transition-all duration-nav ease-nav group touch-manipulation`
          }
        >
          <span className="transition-transform duration-nav ease-nav">{text}</span>
          {({ isActive }) =>
            isActive && (
              <span className="nav-dot w-1.5 h-1.5 rounded-full bg-white" />
            )
          }
        </NavLink>
      ))}
    </div>
  );
}