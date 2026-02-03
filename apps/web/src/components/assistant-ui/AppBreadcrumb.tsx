import { Fragment } from 'react';
import { Link, useRouterState, useParams } from '@tanstack/react-router';
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/components/ui/breadcrumb';

/**
 * Builds breadcrumb segments from current _app route (pathname + params).
 * Renders in the top navbar; only used inside AppLayout (_app routes).
 */
export function AppBreadcrumb() {
  const { pathname } = useRouterState({ select: (s) => s.location });
  const params = useParams({ strict: false });

  const segments = buildBreadcrumbSegments(pathname, params);
  if (segments.length === 0) return null;

  return (
    <Breadcrumb>
      <BreadcrumbList>
        {segments.map((seg, i) => (
          <Fragment key={i}>
            {i > 0 && <BreadcrumbSeparator />}
            <BreadcrumbItem>
              {i < segments.length - 1 ? (
                <BreadcrumbLink asChild>
                  <Link to={seg.href}>{seg.label}</Link>
                </BreadcrumbLink>
              ) : (
                <BreadcrumbPage>{seg.label}</BreadcrumbPage>
              )}
            </BreadcrumbItem>
          </Fragment>
        ))}
      </BreadcrumbList>
    </Breadcrumb>
  );
}

type Segment = { label: string; href: string };

function buildBreadcrumbSegments(
  pathname: string,
  params: Record<string, string | undefined>,
): Segment[] {
  const parts = pathname.replace(/^\/+|\/+$/g, '').split('/').filter(Boolean);
  const segments: Segment[] = [{ label: 'Home', href: '/' }];

  if (parts.length === 0) return segments;

  if (parts[0] === 'feed') {
    segments.push({ label: 'Feed', href: '/feed' });
    return segments;
  }

  if (parts[0] === 'c') {
    segments.push({ label: 'Communities', href: '/c' });
    if (parts[1]) {
      const community = params.community ?? parts[1];
      segments.push({ label: community, href: `/c/${community}` });
    }
    return segments;
  }

  if (parts[0] === 'posts' && parts[1]) {
    const id = params.id ?? parts[1];
    const shortId = id.length > 12 ? `${id.slice(0, 8)}…` : id;
    segments.push({ label: `Post ${shortId}`, href: `/posts/${id}` });
    return segments;
  }

  if (parts[0] === 'event' && parts[1]) {
    const id = params.id ?? parts[1];
    const shortId = id.length > 12 ? `${id.slice(0, 8)}…` : id;
    segments.push({ label: `Event ${shortId}`, href: `/event/${id}` });
    return segments;
  }

  if (parts[0] === 'u' && parts[1]) {
    const npub = params.npub ?? parts[1];
    const shortNpub =
      npub.length > 16 ? `${npub.slice(0, 8)}…${npub.slice(-4)}` : npub;
    segments.push({ label: `Profile ${shortNpub}`, href: `/u/${npub}` });
    return segments;
  }

  return segments;
}
