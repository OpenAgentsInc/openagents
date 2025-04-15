import { AppSidebar } from '@/components/layout/sidebar/app-sidebar';
import { SidebarProvider } from '@/components/ui/sidebar';
import { CreateIssueModalProvider } from '@/components/common/issues/create-issue-modal-provider';
import { cn } from '@/lib/utils';

interface MainLayoutProps {
   children: React.ReactNode;
   header: React.ReactNode;
   headersNumber?: 1 | 2;
}

export default function MainLayout({ children, header, headersNumber = 2 }: MainLayoutProps) {
   const height = {
      1: 'h-[calc(100svh-40px)] lg:h-[calc(100svh-56px)]',
      2: 'h-[calc(100svh-80px)] lg:h-[calc(100svh-96px)]',
   };
   return (
      <SidebarProvider>
         <CreateIssueModalProvider />
         <AppSidebar />
         <div className="h-svh overflow-hidden lg:p-2 w-full">
            <div className="lg:border lg:rounded-md overflow-hidden flex flex-col items-center justify-start bg-container h-full w-full">
               {header}
               <div
                  className={cn(
                     'overflow-auto w-full',
                     height[headersNumber as keyof typeof height]
                  )}
               >
                  {children}
               </div>
            </div>
         </div>
      </SidebarProvider>
   );
}
