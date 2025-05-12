import HeaderNav from './header-nav';
import { SidebarProvider } from '@/components/ui/sidebar';

export default function Header() {
   return (
      <div className="w-full flex flex-col items-center">
         <SidebarProvider>
            <HeaderNav />
         </SidebarProvider>
      </div>
   );
}
