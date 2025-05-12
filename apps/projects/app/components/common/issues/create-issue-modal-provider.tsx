

import { CreateNewIssue } from '@/components/layout/sidebar/create-new-issue';
import { useLoaderData, useParams } from 'react-router';

export function CreateIssueModalProvider() {
  const loaderData = useLoaderData();
  const params = useParams();
  
  // Check if we're in a project context (URL has /projects/:id)
  const currentProjectId = params.id;
  
  return (
    <div className="hidden">
      <CreateNewIssue 
        loaderData={loaderData} 
        initialProjectId={currentProjectId} 
      />
    </div>
  );
}
