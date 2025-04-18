import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { SidebarTrigger } from '@/components/ui/sidebar';
import { useSearchStore } from '@/store/search-store';
import { SearchIcon } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { Notifications } from './notifications';
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbSeparator,
  BreadcrumbPage
} from "@/components/ui/breadcrumb";
import { useParams, useLocation } from 'react-router';

export function HeaderNav() {
  const { isSearchOpen, toggleSearch, closeSearch, setSearchQuery, searchQuery } =
    useSearchStore();
  const searchInputRef = useRef<HTMLInputElement>(null);
  const searchContainerRef = useRef<HTMLDivElement>(null);
  const previousValueRef = useRef<string>('');

  useEffect(() => {
    if (isSearchOpen && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [isSearchOpen]);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        searchContainerRef.current &&
        !searchContainerRef.current.contains(event.target as Node) &&
        isSearchOpen
      ) {
        if (searchQuery.trim() === '') {
          closeSearch();
        }
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isSearchOpen, closeSearch, searchQuery]);

  // Get current path parameters for breadcrumbs
  const params = useParams();
  const location = useLocation();
  
  // Check if we're on an issue page
  const isIssuePage = location.pathname.includes('/issues/') && params.id;
  
  // Get project and issue identifier from path/query params if needed
  // In a real app, you'd get this from a data context or loader data
  
  return (
    <div className="w-full flex justify-between items-center border-b py-1.5 px-6 h-10">
      <div className="flex items-center gap-2">
        <SidebarTrigger className="" />
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium">Issues</span>
          
          {isIssuePage && (
            <Breadcrumb className="ml-2">
              <BreadcrumbList className="text-xs">
                <BreadcrumbItem>
                  <BreadcrumbLink href="/issues" className="text-xs">All</BreadcrumbLink>
                </BreadcrumbItem>
                <BreadcrumbSeparator className="mx-1" />
                <BreadcrumbItem>
                  <BreadcrumbPage className="text-xs font-mono">{params.id}</BreadcrumbPage>
                </BreadcrumbItem>
              </BreadcrumbList>
            </Breadcrumb>
          )}
        </div>
      </div>

      <div className="flex items-center gap-2">
        {isSearchOpen ? (
          <div
            ref={searchContainerRef}
            className="relative flex items-center justify-center w-64 transition-all duration-200 ease-in-out"
          >
            <SearchIcon className="absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground h-4 w-4" />
            <Input
              type="search"
              ref={searchInputRef}
              value={searchQuery}
              onChange={(e) => {
                previousValueRef.current = searchQuery;
                const newValue = e.target.value;
                setSearchQuery(newValue);

                if (previousValueRef.current && newValue === '') {
                  const inputEvent = e.nativeEvent as InputEvent;
                  if (
                    inputEvent.inputType !== 'deleteContentBackward' &&
                    inputEvent.inputType !== 'deleteByCut'
                  ) {
                    closeSearch();
                  }
                }
              }}
              placeholder="Search issues..."
              className="pl-8 h-7 text-sm"
              onKeyDown={(e) => {
                if (e.key === 'Escape') {
                  if (searchQuery.trim() === '') {
                    closeSearch();
                  } else {
                    setSearchQuery('');
                  }
                }
              }}
            />
          </div>
        ) : (
          <>
            {/* <Button
              variant="ghost"
              size="icon"
              onClick={toggleSearch}
              className="h-8 w-8"
              aria-label="Search"
            >
              <SearchIcon className="h-4 w-4" />
            </Button>
            <Notifications /> */}
          </>
        )}
      </div>
    </div>
  );
}
