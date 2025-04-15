'use client';

import { CreateNewIssue } from '@/components/layout/sidebar/create-new-issue';

export function CreateIssueModalProvider() {
   return (
      <div className="hidden">
         <CreateNewIssue />
      </div>
   );
}
