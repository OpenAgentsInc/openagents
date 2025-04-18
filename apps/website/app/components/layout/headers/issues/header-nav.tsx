import { SidebarTrigger } from "@/components/ui/sidebar";
import { Input } from "@/components/ui/input";
import { useSearchStore } from "@/store/search-store";
import { useIssuesStore } from "@/store/issues-store";
import { SearchIcon } from "lucide-react";
import { useEffect, useRef } from "react";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbSeparator,
  BreadcrumbPage,
} from "@/components/ui/breadcrumb";
import { useParams, useLocation } from "react-router";

export function HeaderNav() {
  /* ---------- search state ---------- */
  const {
    isSearchOpen,
    toggleSearch,
    closeSearch,
    setSearchQuery,
    searchQuery,
  } = useSearchStore();
  const searchInputRef = useRef<HTMLInputElement>(null);
  const searchContainerRef = useRef<HTMLDivElement>(null);
  const previousValueRef = useRef("");

  useEffect(() => {
    if (isSearchOpen && searchInputRef.current) searchInputRef.current.focus();
  }, [isSearchOpen]);

  useEffect(() => {
    const clickOutside = (e: MouseEvent) => {
      if (
        searchContainerRef.current &&
        !searchContainerRef.current.contains(e.target as Node) &&
        isSearchOpen &&
        searchQuery.trim() === ""
      ) {
        closeSearch();
      }
    };
    document.addEventListener("mousedown", clickOutside);
    return () => document.removeEventListener("mousedown", clickOutside);
  }, [isSearchOpen, closeSearch, searchQuery]);

  /* ---------- breadcrumbs ---------- */
  const params = useParams();
  const location = useLocation();
  const isIssuePage = location.pathname.includes("/issues/") && params.id;
  const issueId = params.id as string | undefined;
  
  /* ---------- issue data ---------- */
  const { getIssueById } = useIssuesStore();
  const currentIssue = isIssuePage && issueId ? getIssueById(issueId) : undefined;

  return (
    <div className="w-full h-full flex items-center justify-between px-6 text-xs">
      {/* -------- left: sidebar + breadcrumb -------- */}
      <div className="flex items-center gap-3">
        <SidebarTrigger />

        <Breadcrumb>
          <BreadcrumbList>
            <BreadcrumbItem>
              {isIssuePage && currentIssue?.project ? (
                <BreadcrumbLink href={`/projects/${currentIssue.project.id}`}>
                  {currentIssue.project.name}
                </BreadcrumbLink>
              ) : (
                <BreadcrumbPage>Issues</BreadcrumbPage>
              )}
            </BreadcrumbItem>

            {isIssuePage && (
              <>
                <BreadcrumbSeparator />
                <BreadcrumbItem>
                  <BreadcrumbPage title={currentIssue?.title || ""} className="font-mono">
                    {currentIssue?.identifier || issueId}
                  </BreadcrumbPage>
                </BreadcrumbItem>
              </>
            )}
          </BreadcrumbList>
        </Breadcrumb>
      </div>

      {/* -------- right: (optional) search -------- */}
      <div className="flex items-center gap-2">
        {isSearchOpen ? (
          <div
            ref={searchContainerRef}
            className="relative flex items-center w-64"
          >
            <SearchIcon className="absolute left-2 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
            <Input
              ref={searchInputRef}
              type="search"
              value={searchQuery}
              onChange={(e) => {
                previousValueRef.current = searchQuery;
                const v = e.target.value;
                setSearchQuery(v);

                if (previousValueRef.current && v === "") {
                  const ev = e.nativeEvent as InputEvent;
                  if (
                    ev.inputType !== "deleteContentBackward" &&
                    ev.inputType !== "deleteByCut"
                  ) {
                    closeSearch();
                  }
                }
              }}
              placeholder="Search issues…"
              className="pl-8 h-7 text-sm"
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  searchQuery.trim() === "" ? closeSearch() : setSearchQuery("");
                }
              }}
            />
          </div>
        ) : (
          /* Hook up your search / notification buttons here if desired */
          <></>
        )}
      </div>
    </div>
  );
}
