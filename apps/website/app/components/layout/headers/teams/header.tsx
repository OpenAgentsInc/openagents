import HeaderNav from './header-nav';
import HeaderOptions from './header-options';

export function Header() {
  return (
    <div className="w-full flex flex-col items-center">
      <div className="container mx-auto py-4 px-6 flex items-center justify-between">
        <h1 className="text-2xl font-bold">Teams</h1>
      </div>
      <HeaderNav />
      <HeaderOptions />
    </div>
  );
}

export default Header;