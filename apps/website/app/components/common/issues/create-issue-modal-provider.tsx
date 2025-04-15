

import { CreateNewIssue } from '@/components/layout/sidebar/create-new-issue';
import { useLoaderData } from 'react-router';

export function CreateIssueModalProvider() {
  const loaderData = useLoaderData();
  
  return (
    <div className="hidden">
      <CreateNewIssue loaderData={loaderData} />
    </div>
  );
}
