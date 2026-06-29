import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import type {
  ForumForumRoute,
  ForumReceiptRoute,
  ForumRoute,
  ForumTopicRoute,
} from '../route'
import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

type ForumRouteValue =
  | ForumRoute
  | ForumForumRoute
  | ForumTopicRoute
  | ForumReceiptRoute
type ForumAuthMode = PublicHeaderAuthState<unknown>['_tag']

const shellClass =
  'h-dvh overflow-auto overscroll-contain bg-forum-page text-forum-text'
const containerClass =
  'mx-auto grid w-[min(100%,1180px)] gap-4 border-x border-forum-wrap-border bg-forum-wrap px-3 py-4 font-sans shadow-[0_0_0_1px_rgba(237,237,237,0.8)] sm:px-4 sm:py-5'
const panelClass = 'rounded-md border border-forum-row-c bg-forum-panel'
const eyebrowClass = 'font-sans text-xs font-bold uppercase text-forum-heading'
const mutedClass = 'text-sm/6 text-forum-text'
const ghostButtonClass =
  'min-h-8 rounded border border-forum-row-c bg-forum-panel px-3 py-1.5 text-sm font-bold text-forum-link hover:border-forum-header hover:bg-forum-post-link-hover-bg hover:text-forum-link-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-forum-header'
const forumHeaderClass =
  'rounded-md bg-forum-header px-3 py-2 text-sm font-bold text-white'
const forumBreadcrumbClass =
  'rounded-md bg-forum-navbar px-3 py-2 text-sm text-forum-heading'

const forumReturnPath = (route: ForumRouteValue): string =>
  route._tag === 'ForumForum'
    ? `/forum/f/${encodeURIComponent(route.forumRef)}`
    : route._tag === 'ForumTopic'
      ? `/forum/t/${encodeURIComponent(route.topicId)}`
      : route._tag === 'ForumReceipt'
        ? `/forum/receipts/${encodeURIComponent(route.receiptRef)}`
        : '/forum'

const forumLoginHref = (route: ForumRouteValue): string =>
  `/login/github?returnTo=${encodeURIComponent(forumReturnPath(route))}`

export const forumScript = (
  route: ForumRouteValue,
  authMode: ForumAuthMode = 'LoggedOut',
): string => {
  const loginHref = forumLoginHref(route)
  const initial =
    route._tag === 'ForumForum'
      ? { authMode, kind: 'forum', ref: route.forumRef }
      : route._tag === 'ForumTopic'
        ? { authMode, kind: 'topic', id: route.topicId }
        : route._tag === 'ForumReceipt'
          ? { authMode, kind: 'receipt', ref: route.receiptRef }
          : { authMode, kind: 'index' }

  return `(() => {
  const initial = ${JSON.stringify(initial)};
  const authMode = initial.authMode || 'LoggedOut';
  const loginHref = ${JSON.stringify(loginHref)};
  // ---- Forum theme (light / dark / system) ----
  // Preference is stored as 'light' | 'dark'; absence means "follow system".
  // The resolved theme is written to <html data-forum-theme="..."> which the
  // stylesheet keys the forum's --color-forum-* tokens off of. This runs
  // before the root/main guard so the theme always applies, even on pages
  // that re-render their own content.
  const THEME_KEY = 'oa.forum.v1:theme';
  const themeMedia = window.matchMedia('(prefers-color-scheme: dark)');
  const readThemePref = () => {
    try {
      const stored = localStorage.getItem(THEME_KEY);
      return stored === 'light' || stored === 'dark' ? stored : 'system';
    } catch (_) {
      return 'system';
    }
  };
  const resolveTheme = pref =>
    pref === 'light' || pref === 'dark'
      ? pref
      : themeMedia.matches ? 'dark' : 'light';
  const applyTheme = pref => {
    document.documentElement.setAttribute('data-forum-theme', resolveTheme(pref));
  };
  const syncThemeSelect = pref => {
    const select = document.querySelector('[data-forum-theme-select]');
    if (select && select.value !== pref) select.value = pref;
  };
  applyTheme(readThemePref());
  syncThemeSelect(readThemePref());
  document.addEventListener('change', event => {
    const target = event.target;
    const select = target && target.closest ? target.closest('[data-forum-theme-select]') : null;
    if (!select) return;
    const value = select.value;
    const pref = value === 'light' || value === 'dark' ? value : 'system';
    try {
      if (pref === 'system') localStorage.removeItem(THEME_KEY);
      else localStorage.setItem(THEME_KEY, pref);
    } catch (_) {}
    applyTheme(pref);
  });
  themeMedia.addEventListener('change', () => {
    if (readThemePref() === 'system') applyTheme('system');
  });
  const state = {
    forum: null,
    launchStatus: null,
    topics: [],
    topic: null,
    posts: [],
    topicPostSortDirection: 'asc',
    receipt: null,
    tipLeaderboards: null,
  };
  const root = document.querySelector('[data-forum-app]');
  const main = document.querySelector('[data-forum-main]');
  if (!root || !main) return;

  const escapeHtml = value => String(value ?? '').replace(/[&<>"']/g, char => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  })[char]);
  const takeLoginError = () => {
    const found = document.cookie.split(';').map(part => part.trim()).includes('oa_login_error=github_login_failed');
    if (found) {
      document.cookie = 'oa_login_error=; Max-Age=0; Path=/; Secure; SameSite=Lax';
      document.cookie = 'oa_login_error=; Max-Age=0; Path=/; SameSite=Lax';
    }
    return found;
  };
  const loginErrorRoot = document.querySelector('[data-forum-login-error]');
  if (loginErrorRoot && takeLoginError()) {
    loginErrorRoot.innerHTML = '<section class="${panelClass} p-3 sm:p-4" role="alert"><p class="${eyebrowClass}">Login failed</p><p class="${mutedClass}">GitHub login did not complete. Try again.</p></section>';
  }
  const friendlyTime = value => {
    if (!value) return 'Unknown time';
    const timestamp = Date.parse(value);
    if (Number.isNaN(timestamp)) return 'Unknown time';
    const seconds = Math.round((performance.timeOrigin + performance.now() - timestamp) / 1000);
    const abs = Math.abs(seconds);
    if (abs < 60) return seconds < 0 ? 'just now' : 'just now';
    const minutes = Math.round(abs / 60);
    if (minutes < 60) return seconds < 0 ? 'in ' + minutes + ' min' : minutes + ' min ago';
    const hours = Math.round(minutes / 60);
    if (hours < 24) return seconds < 0 ? 'in ' + hours + ' hr' : hours + ' hr ago';
    return new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric', year: 'numeric' }).format(timestamp);
  };
  const api = async (path, options = {}) => {
    const response = await fetch(path, {
      ...options,
      credentials: 'same-origin',
      headers: {
        ...(options.body ? { 'Content-Type': 'application/json' } : {}),
        ...(options.headers || {}),
      },
    });
    const body = await response.json().catch(() => ({}));
    if (!response.ok) {
      const reason = body.reason || body.error || 'Request failed';
      throw new Error(reason);
    }
    return body;
  };
  // Stale-while-revalidate cache (localStorage): pages render instantly
  // from the last-seen payloads, then refresh in the background. The
  // Loading panel only ever appears on a truly cold first visit.
  const CACHE_PREFIX = 'oa.forum.v1:';
  const CACHE_TTL_MS = 10 * 60 * 1000;
  const parseCacheEntry = JSON.parse.bind(JSON);
  const nowMs = () => performance.timeOrigin + performance.now();
  const cacheGet = key => {
    try {
      const raw = localStorage.getItem(CACHE_PREFIX + key);
      if (!raw) return null;
      const entry = parseCacheEntry(raw);
      if (!entry || typeof entry.t !== 'number') return null;
      if (nowMs() - entry.t > CACHE_TTL_MS) return null;
      return entry.d;
    } catch { return null; }
  };
  const cacheSet = (key, data) => {
    try { localStorage.setItem(CACHE_PREFIX + key, JSON.stringify({ t: nowMs(), d: data })); } catch {}
  };
  const forumHref = forum => '/forum/f/' + encodeURIComponent(forum.slug || forum.forumId);
  const topicHref = topic => '/forum/t/' + encodeURIComponent(topic.topicId);
  const topicSortQuery = direction => '?sortDir=' + (direction === 'desc' ? 'desc' : 'asc');
  const topicSortHref = (topic, direction) => topicHref(topic) + topicSortQuery(direction);
  const topicApiPath = topicId => '/api/forum/topics/' + encodeURIComponent(topicId) + (state.topicPostSortDirection === 'desc' ? '?sortDir=desc' : '');
  const initialTopicPostSortDirection = () => {
    const params = new URLSearchParams(window.location.search);
    const sortDir = String(params.get('sortDir') || '').trim().toLowerCase();
    if (sortDir === 'desc') return 'desc';
    if (sortDir === 'asc') return 'asc';
    const sd = String(params.get('sd') || '').trim().toLowerCase();
    return sd === 'd' ? 'desc' : 'asc';
  };
  if (initial.kind === 'topic') state.topicPostSortDirection = initialTopicPostSortDirection();
  const postAnchor = post => 'post-' + encodeURIComponent(post.postId);
  const postNumberAnchor = post => 'post-' + Number(post.postNumber || 0);
  const postHref = post => topicHref(post) + '#' + postAnchor(post);
  const defaultPostRewardSats = 10;
  const postRewardAmountLabel = amount => String(amount) + ' sats';
  const postRewardCaveat = 'Content reward; receipt separates payment from settlement.';
  const countText = (count, singular, plural) => {
    const normalized = Number(count || 0);
    return normalized === 1 ? '1 ' + singular : normalized + ' ' + plural;
  };
  const postTipStatsBadge = post => {
    const stats = post.tipStats || {};
    const totalPaidSats = Number(stats.totalPaidSats || 0);
    if (!Number.isFinite(totalPaidSats) || totalPaidSats <= 0) return '';
    const rawSettledSats = Number(stats.totalSettledSats || 0);
    const totalSettledSats = Number.isFinite(rawSettledSats) ? rawSettledSats : 0;
    const tipCount = Number(stats.tipCount || 0);
    const settlement = totalSettledSats >= totalPaidSats ? 'settled' : totalSettledSats > 0 ? 'partial' : 'pending';
    const settlementIcon = settlement === 'settled' ? '✓' : '◷';
    const detail = String(totalPaidSats) + ' sats paid · ' + String(totalSettledSats) + ' sats settled' +
      (settlement === 'settled' ? '' : ' · settlement pending') +
      (tipCount > 1 ? ' · ' + String(tipCount) + ' payments' : '');
    return '<span data-forum-post-tip-total data-forum-post-tip-settlement="' + settlement + '" class="font-sans text-sm text-forum-payment sm:text-xs" title="' + escapeHtml(detail) + '" aria-label="' + escapeHtml(detail) + '">' + escapeHtml(String(totalPaidSats) + ' sats ' + settlementIcon) + '</span>';
  };
  const markdownParagraphClass = 'm-0 break-words text-sm/6 text-forum-heading [overflow-wrap:anywhere]';
  const markdownHeadingClass = 'm-0 break-words pt-1 font-semibold text-forum-heading [overflow-wrap:anywhere]';
  const markdownLinkClass = 'text-forum-link underline underline-offset-4 hover:text-forum-link-hover';
  const safeMarkdownHref = href => {
    const trimmed = String(href || '').trim();
    if (trimmed.startsWith('/') && !trimmed.startsWith('//')) return trimmed;
    try {
      const parsed = new URL(trimmed, window.location.origin);
      return parsed.protocol === 'http:' || parsed.protocol === 'https:' ? parsed.href : '';
    } catch {
      return '';
    }
  };
  const renderInlineEmphasis = value =>
    escapeHtml(value)
      .replace(/\\*\\*([^*]+)\\*\\*/g, (_match, body) => '<strong class="font-semibold text-forum-heading">' + body + '</strong>')
      .replace(/__([^_]+)__/g, (_match, body) => '<strong class="font-semibold text-forum-heading">' + body + '</strong>')
      .replace(/(^|[\\s(])\\*([^*]+)\\*/g, (_match, prefix, body) => prefix + '<em class="italic">' + body + '</em>')
      .replace(/(^|[\\s(])_([^_]+)_/g, (_match, prefix, body) => prefix + '<em class="italic">' + body + '</em>');
  const renderInlineWithoutLinks = value => {
    const delimiter = String.fromCharCode(96);
    const segments = String(value || '').split(delimiter);
    if (segments.length % 2 === 0) return renderInlineEmphasis(value);
    return segments.map((segment, index) =>
      index % 2 === 1
        ? '<code class="rounded border border-forum-row-c bg-forum-panel px-1 py-0.5 font-mono text-[0.8125rem] text-forum-heading">' + escapeHtml(segment) + '</code>'
        : renderInlineEmphasis(segment)
    ).join('');
  };
  const renderInlineMarkdown = value => {
    const text = String(value || '');
    const linkPattern = /\\[([^\\]\\n]+)\\]\\(([^)\\s]+)\\)/g;
    let cursor = 0;
    let html = '';
    for (const match of text.matchAll(linkPattern)) {
      const index = match.index ?? 0;
      html += renderInlineWithoutLinks(text.slice(cursor, index));
      const label = match[1] || match[2] || 'Link';
      const href = safeMarkdownHref(match[2]);
      html += href === ''
        ? renderInlineWithoutLinks(label)
        : '<a class="' + markdownLinkClass + '" href="' + escapeHtml(href) + '"' + (href.startsWith('/') ? '' : ' target="_blank" rel="noreferrer"') + '>' + renderInlineWithoutLinks(label) + '</a>';
      cursor = index + match[0].length;
    }
    return html + renderInlineWithoutLinks(text.slice(cursor));
  };
  const isFenceLine = line => {
    const trimmed = line.trim();
    return trimmed.startsWith('\`\`\`') || trimmed.startsWith('~~~');
  };
  const isMarkdownBoundary = line => {
    const trimmed = line.trim();
    return trimmed === '' ||
      isFenceLine(line) ||
      /^#{1,6}\\s+/.test(trimmed) ||
      /^[-*_]{3,}$/.test(trimmed) ||
      /^\\s*>/.test(line) ||
      /^\\s*(?:[-*+]\\s+|\\d+[.)]\\s+)/.test(line);
  };
  const renderMarkdownBlock = (tag, text, extraClass = '') =>
    '<' + tag + ' class="' + (extraClass || markdownParagraphClass) + '">' + renderInlineMarkdown(text) + '</' + tag + '>';
  const renderMarkdownList = (lines, startIndex, ordered) => {
    let index = startIndex;
    const items = [];
    let startNumber = 1;
    const pattern = ordered
      ? /^\\s*(\\d+)[.)]\\s+(.+)$/
      : /^\\s*[-*+]\\s+(.+)$/;
    const nextContentLineIndex = candidate =>
      candidate < lines.length && (lines[candidate] || '').trim() === ''
        ? nextContentLineIndex(candidate + 1)
        : candidate;
    while (index < lines.length) {
      const match = pattern.exec(lines[index] || '');
      if (!match) break;
      if (ordered && items.length === 0) {
        const parsed = Number(match[1]);
        startNumber = Number.isFinite(parsed) && parsed > 0 ? parsed : 1;
      }
      const body = ordered ? match[2] : match[1];
      items.push('<li class="pl-1">' + renderInlineMarkdown(body || '') + '</li>');
      index += 1;
      const nextIndex = nextContentLineIndex(index);
      if (nextIndex !== index && pattern.test(lines[nextIndex] || '')) {
        index = nextIndex;
      }
    }
    const tag = ordered ? 'ol' : 'ul';
    const className = ordered
      ? 'm-0 list-decimal space-y-1 pl-6 text-sm/6 text-forum-heading'
      : 'm-0 list-disc space-y-1 pl-6 text-sm/6 text-forum-heading';
    const startAttribute = ordered && startNumber > 1 ? ' start="' + String(startNumber) + '"' : '';
    return { html: '<' + tag + startAttribute + ' class="' + className + '">' + items.join('') + '</' + tag + '>', nextIndex: index };
  };
  const renderMarkdown = value => {
    const lines = String(value || '').replace(/\\r\\n?/g, '\\n').split('\\n');
    const blocks = [];
    let index = 0;
    while (index < lines.length) {
      const line = lines[index] || '';
      const trimmed = line.trim();
      if (trimmed === '') {
        index += 1;
        continue;
      }
      if (isFenceLine(line)) {
        const fence = trimmed.slice(0, 3);
        const codeLines = [];
        index += 1;
        while (index < lines.length && !(lines[index] || '').trim().startsWith(fence)) {
          codeLines.push(lines[index] || '');
          index += 1;
        }
        if (index < lines.length) index += 1;
        blocks.push('<pre class="m-0 overflow-x-auto rounded border border-forum-row-c bg-forum-wrap p-3 text-xs/6 text-forum-heading"><code>' + escapeHtml(codeLines.join('\\n')) + '</code></pre>');
        continue;
      }
      const heading = /^(#{1,6})\\s+(.+)$/.exec(trimmed);
      if (heading) {
        const depth = Math.min((heading[1] || '').length + 3, 6);
        const sizeClass = depth <= 4 ? 'text-base' : 'text-sm';
        blocks.push(renderMarkdownBlock('h' + depth, heading[2] || '', markdownHeadingClass + ' ' + sizeClass));
        index += 1;
        continue;
      }
      if (/^[-*_]{3,}$/.test(trimmed)) {
        blocks.push('<hr class="m-0 border-forum-row-c">');
        index += 1;
        continue;
      }
      if (/^\\s*>/.test(line)) {
        const quoteLines = [];
        while (index < lines.length && /^\\s*>/.test(lines[index] || '')) {
          quoteLines.push((lines[index] || '').replace(/^\\s*>\\s?/, '').trim());
          index += 1;
        }
        blocks.push('<blockquote class="m-0 border-l-4 border-forum-header bg-forum-panel px-3 py-2 text-sm/6 text-forum-text">' + renderInlineMarkdown(quoteLines.join(' ')) + '</blockquote>');
        continue;
      }
      const ordered = /^\\s*\\d+[.)]\\s+/.test(line);
      const unordered = /^\\s*[-*+]\\s+/.test(line);
      if (ordered || unordered) {
        const list = renderMarkdownList(lines, index, ordered);
        blocks.push(list.html);
        index = list.nextIndex;
        continue;
      }
      const paragraphLines = [];
      while (index < lines.length && !isMarkdownBoundary(lines[index] || '')) {
        paragraphLines.push((lines[index] || '').trim());
        index += 1;
      }
      blocks.push(renderMarkdownBlock('p', paragraphLines.join(' ')));
    }
    return blocks.length === 0
      ? '<p class="' + markdownParagraphClass + '"></p>'
      : blocks.join('');
  };
  const topicCountText = count => countText(count, 'topic', 'topics');
  const postCountText = count => countText(count, 'post', 'posts');
  const replyCountText = count => countText(count, 'reply', 'replies');
  const viewCountText = count => countText(count, 'view', 'views');
  const rowClass = index => index % 2 === 0 ? 'bg-forum-row-a' : 'bg-forum-row-b';
  const forumListClass = 'overflow-hidden rounded-md border border-forum-row-c bg-forum-wrap';
  const listHeaderClass = 'hidden border-b border-forum-header bg-forum-header px-3 py-2 text-xs font-bold uppercase text-white sm:grid';
  const forumGridClass = 'sm:grid-cols-[2.5rem_minmax(0,1fr)_5.5rem_5.5rem_16rem]';
  const topicGridClass = 'sm:grid-cols-[2.5rem_minmax(0,1fr)_5.5rem_5.5rem_16rem]';
  const metadataCellClass = 'hidden items-center justify-center border-l border-forum-row-c px-2 py-3 text-center font-sans text-xs text-forum-text sm:flex';
  const lastPostCellClass = 'hidden min-w-0 border-l border-forum-row-c px-3 py-3 text-xs text-forum-text sm:block';
  const compactMeta = html => '<div class="mt-2 flex flex-wrap gap-2 text-xs text-forum-text sm:hidden">' + html + '</div>';
  const compactChip = value => '<span class="rounded border border-forum-row-c bg-forum-panel px-2 py-1">' + value + '</span>';
  const listHeader = (gridClass, first, countA, countB, last) =>
    '<div class="' + listHeaderClass + ' ' + gridClass + '">' +
      '<span></span><span>' + first + '</span><span class="text-center">' + countA + '</span><span class="text-center">' + countB + '</span><span>' + last + '</span>' +
    '</div>';
  // Prosilver-style forum icon: a small rounded square with a speech
  // bubble glyph, instead of the old oversized circles.
  const statusMarker = label =>
    '<span class="flex size-9 flex-col items-center justify-center gap-[3px] rounded border border-forum-row-c bg-gradient-to-b from-white to-forum-row-a shadow-sm" title="' + escapeHtml(label) + '" aria-label="' + escapeHtml(label) + '">' +
      '<span class="block h-[3px] w-4 rounded-full bg-forum-header"></span>' +
      '<span class="block h-[3px] w-4 rounded-full bg-forum-header/70"></span>' +
      '<span class="block h-[3px] w-3 self-start ml-[10px] rounded-full bg-forum-header/45"></span>' +
    '</span>';
  const forumStatusLabel = forum => forum.locked
    ? 'Locked forum'
    : forum.discoverability === 'unlisted'
      ? 'Unlisted forum'
      : 'Listed forum';
  const topicStatusLabel = topic => topic.locked || topic.state === 'locked'
    ? 'Locked topic'
    : topic.topicType === 'sticky' || topic.sticky
      ? 'Sticky topic'
      : topic.topicType === 'announcement' || topic.announcement
        ? 'Announcement topic'
        : 'Topic';
  const lastPostProjection = item => item.lastPost || item.lastPostSummary || item.latestPost || null;
  const lastPostCell = item => {
    const lastPost = lastPostProjection(item);
    if (!lastPost) {
      return '<span class="text-forum-text">No posts</span>';
    }
    const subject = lastPost.subject || lastPost.title || lastPost.topicTitle || 'Last post';
    const author = lastPost.author?.displayName || lastPost.author?.actorRef || lastPost.authorDisplayName || lastPost.actorRef || 'Unknown';
    const time = friendlyTime(lastPost.createdAt || lastPost.updatedAt || lastPost.timestamp);
    // This cell lives inside the row's own <a>; a nested anchor here
    // makes the browser split the outer anchor and eject this cell out
    // of the grid as a full-width sibling (the broken white bands).
    // The subject stays a span; the row link covers navigation.
    const truncated = subject.length > 48 ? subject.slice(0, 47).trimEnd() + '…' : subject;
    const subjectHtml = '<span class="block truncate font-bold text-forum-heading" title="' + escapeHtml(subject) + '">' + escapeHtml(truncated) + '</span>';
    // NOTE: this cell renders inside anchor rows; author names stay
    // plain text here to avoid nested <a>. Linked names appear on the
    // leaderboards, thread sidebars, and tip controls instead.
    return subjectHtml + '<span class="block truncate">by ' + escapeHtml(author) + ' &raquo; ' + escapeHtml(time) + '</span>';
  };
  const actionBar = html => '<div class="flex flex-wrap items-center justify-between gap-2 rounded-md bg-forum-navbar px-3 py-2 text-xs text-forum-heading">' + html + '</div>';
  const pageSummary = (total, label) => '<span class="font-bold">' + countText(total, label, label + 's') + ' &bull; Page 1</span>';
  const actorDisplayName = actor => actor?.displayName || actor?.actorRef || 'Unknown';
  const actorProfileHref = actor =>
    actor?.actorId && actor?.slug
      ? '/forum/u/' + encodeURIComponent(actor.actorId) + '/' + encodeURIComponent(actor.slug)
      : null;
  const actorNameHtml = (actor, extraClass = '') => {
    const name = escapeHtml(actorDisplayName(actor));
    const href = actorProfileHref(actor);
    return href
      ? '<a class="text-forum-link hover:text-forum-link-hover hover:underline ' + extraClass + '" href="' + escapeHtml(href) + '">' + name + '</a>'
      : '<span class="' + extraClass + '">' + name + '</span>';
  };
  const actorInitial = actor => actorDisplayName(actor).trim().slice(0, 1).toUpperCase() || 'A';
  const actorRole = actor => actor?.role || actor?.rank || actor?.kind || 'Member';
  const forumBadge = forum => forum.discoverability === 'unlisted'
    ? '<span class="rounded border border-forum-payment px-2 py-1 font-sans text-xs font-bold text-forum-payment">Unlisted</span>'
    : '<span class="rounded border border-forum-row-c px-2 py-1 font-sans text-xs font-bold text-forum-text">Listed</span>';
  const tipTotalsLabel = item => {
    const paid = Number(item?.totalPaidSats || 0);
    const settled = Number(item?.totalSettledSats || 0);
    const tipCount = Number(item?.tipCount || 0);
    const paidLabel = Number.isFinite(paid) ? String(paid) : '0';
    const settledLabel = Number.isFinite(settled) ? String(settled) : '0';
    return paidLabel + ' paid sats · ' + settledLabel + ' settled sats · ' + tipCount + ' tips';
  };
  const renderTipLeaderboards = leaderboards => {
    const posts = (leaderboards?.posts || []).slice(0, 5);
    const creators = (leaderboards?.creators || []).slice(0, 5);
    if (posts.length === 0 && creators.length === 0) return '';
    const truncatedPostTitle = post => {
      const title = String(post.postTitle || '').trim() ||
        post.author?.displayName || post.author?.actorRef || 'creator';
      return title.length > 70 ? title.slice(0, 69).trimEnd() + '…' : title;
    };
    const postRows = posts.length === 0
      ? '<div class="border-t border-forum-row-c py-3 text-sm text-forum-text">No tipped posts yet.</div>'
      : posts.map(post => '<a class="grid gap-1 border-t border-forum-row-c py-3 text-forum-link hover:bg-forum-post-link-hover-bg hover:text-forum-link-hover" href="' + escapeHtml(post.postPermalink) + '">' +
          '<span class="text-sm font-bold">' + escapeHtml(truncatedPostTitle(post)) + '</span>' +
          '<span class="font-sans text-sm text-forum-payment">' + escapeHtml(tipTotalsLabel(post)) + '</span></a>').join('');
    const creatorRows = creators.length === 0
      ? '<div class="border-t border-forum-row-c py-3 text-sm text-forum-text">No tipped creators yet.</div>'
      : creators.map(creator => '<div class="grid gap-1 border-t border-forum-row-c py-3">' +
          '<span class="text-sm font-bold">' + actorNameHtml(creator.actor) + '</span>' +
          '<span class="font-sans text-sm text-forum-payment">' + escapeHtml(tipTotalsLabel(creator)) + '</span></div>').join('');
    return '<section class="${panelClass} p-4 sm:p-5"><div class="grid gap-5 md:grid-cols-2">' +
      '<div><p class="${eyebrowClass}">Top tipped posts</p><div class="mt-3">' + postRows + '</div></div>' +
      '<div><p class="${eyebrowClass}">Top tipped creators</p><div class="mt-3">' + creatorRows + '</div></div>' +
      '</div></section>';
  };
  const scrollPostAnchorIntoView = () => {
    const rawHash = window.location.hash || '';
    if (!rawHash.startsWith('#post-')) return;
    let anchor = rawHash.slice(1);
    try { anchor = decodeURIComponent(anchor); } catch {}
    const target = document.getElementById(anchor) ||
      Array.from(main.querySelectorAll('[data-forum-post-number-anchor]')).find(element =>
        element.getAttribute('data-forum-post-number-anchor') === anchor
      );
    if (!target) return;
    target.focus({ preventScroll: true });
    target.scrollIntoView({ block: 'start' });
  };
  main.addEventListener('click', event => {
    const button = event.target.closest('[data-forum-copy-permalink]');
    if (!button) return;
    const href = button.getAttribute('data-forum-copy-permalink') || '';
    const absolute = window.location.origin + href;
    navigator.clipboard?.writeText(absolute);
    const original = button.textContent;
    button.textContent = 'Copied';
    setTimeout(() => { button.textContent = original; }, 1500);
  });
  window.addEventListener('hashchange', scrollPostAnchorIntoView);

  const forumRows = forums => forums.length === 0
    ? '<div class="border-t border-forum-row-c px-3 py-8 text-sm text-forum-text">No listed forums yet.</div>'
    : forums.map((forum, index) => {
        const topicCount = Number(forum.topicCount || 0);
        const postCount = Number(forum.postCount || 0);
        return '<a class="grid gap-2 border-t border-forum-row-c px-3 py-4 text-forum-link hover:bg-forum-post-link-hover-bg hover:text-forum-link-hover sm:gap-0 ' + rowClass(index) + ' ' + forumGridClass + '" href="' + forumHref(forum) + '">' +
          '<span class="hidden items-center sm:flex">' + statusMarker(forumStatusLabel(forum)) + '</span>' +
          '<div class="min-w-0"><span class="text-sm font-bold">' + escapeHtml(forum.title) + '</span><span class="ml-2 text-sm text-forum-text">' + escapeHtml(forum.slug || forum.forumId || '') + '</span>' +
          '<span class="mt-1 block text-xs text-forum-text">' + escapeHtml(forum.description || forum.summary || forumStatusLabel(forum)) + '</span>' +
          compactMeta(compactChip(topicCountText(topicCount)) + compactChip(postCountText(postCount)) + compactChip('Last post: ' + (lastPostProjection(forum) ? 'available' : 'No posts'))) + '</div>' +
          '<span class="' + metadataCellClass + '">' + String(topicCount) + '</span>' +
          '<span class="' + metadataCellClass + '">' + String(postCount) + '</span>' +
          '<div class="' + lastPostCellClass + '">' + lastPostCell(forum) + '</div>' +
          '</a>';
      }).join('');
  const topicRows = topics => topics.length === 0
    ? '<div class="border-t border-forum-row-c px-3 py-8 text-sm text-forum-text">No topics yet.</div>'
    : topics.map((topic, index) => {
        const postCount = Number(topic.postCount || 0);
        const replies = Number(topic.replyCount ?? Math.max(postCount - 1, 0));
        const views = Number(topic.viewCount || topic.views || 0);
        return '<a class="grid gap-2 border-t border-forum-row-c px-3 py-4 text-forum-link hover:bg-forum-post-link-hover-bg hover:text-forum-link-hover sm:gap-0 ' + rowClass(index) + ' ' + topicGridClass + '" href="' + topicHref(topic) + '">' +
          '<span class="hidden items-center sm:flex">' + statusMarker(topicStatusLabel(topic)) + '</span>' +
          '<div class="min-w-0"><span class="text-sm font-bold">' + escapeHtml(topic.title) + '</span>' +
          '<span class="mt-1 block text-xs text-forum-text">by ' + escapeHtml(topic.author?.displayName || 'Unknown') + ' &raquo; ' + friendlyTime(topic.createdAt || topic.updatedAt) + '</span>' +
          compactMeta(compactChip(replyCountText(replies)) + compactChip(viewCountText(views)) + compactChip('Last post: ' + (lastPostProjection(topic) ? 'available' : friendlyTime(topic.updatedAt)))) + '</div>' +
          '<span class="' + metadataCellClass + '">' + String(replies) + '</span>' +
          '<span class="' + metadataCellClass + '">' + String(views) + '</span>' +
          '<div class="' + lastPostCellClass + '">' + lastPostCell(topic) + '</div>' +
          '</a>';
      }).join('');
  const tipPostGateReady = () => state.launchStatus?.publicTipping?.postTips === 'ready';
  const firstTipBlocker = () => state.launchStatus?.publicTipping?.remainingBeforeLiveTips?.[0] || 'payment verification';
  const tipGateStatusLabel = () => {
    const blocker = firstTipBlocker().toLowerCase();
    if (blocker.includes('payer wallet')) return 'Tip setup pending';
    if (blocker.includes('smoke')) return 'Live smoke pending';
    return 'Self-serve tips pending';
  };
  const tipStateLabel = value => ({
    dispatched: 'Payout dispatched',
    evidence_only: 'Receipt evidence only',
    failed: 'Payment failed',
    paid: 'Payment recorded',
    payment_required: 'Payment required',
    previewed: 'Previewed',
    recipient_pending: 'Creator settlement pending',
    refunded: 'Refunded',
    reversed: 'Reversed',
    settled: 'Recipient wallet paid',
  })[value] || 'Payment state';
  const findTipPanel = postId =>
    Array.from(main.querySelectorAll('[data-forum-tip-panel]')).find(element =>
      element.getAttribute('data-forum-tip-panel') === postId
    ) || null;
  const setTipPanel = (postId, stateName, html) => {
    const panel = findTipPanel(postId);
    if (!panel) return;
    panel.setAttribute('data-forum-tip-result', stateName);
    panel.innerHTML = html;
  };
  const tipSessionNonce = String(Math.trunc(performance.timeOrigin));
  let tipSequence = 0;
  const tipIdempotencyKey = postId => {
    tipSequence += 1;
    const nonce = tipSessionNonce + ':' + String(tipSequence);
    return 'forum:browser_tip:' + postId + ':' + nonce;
  };
  const renderTipControls = post => {
    const readiness = post.tipRecipientReadiness || {};
    const postId = post.postId || '';
    const recipient = post.author?.displayName || 'creator';
    const stats = post.tipStats || {};
    const totalPaidSats = Number(stats.totalPaidSats || 0);
    if (!tipPostGateReady()) {
      if (Number.isFinite(totalPaidSats) && totalPaidSats > 0) return '';
      return '<span data-forum-tip-state="gated" class="font-sans text-xs text-forum-text" title="' + escapeHtml(firstTipBlocker()) + '">' + escapeHtml(tipGateStatusLabel()) + '</span>';
    }
    if (readiness.tippingAvailable !== true) {
      return '<span data-forum-tip-state="recipient_not_ready" class="font-sans text-xs text-forum-text" title="' + escapeHtml(readiness.blockerRef || 'recipient wallet pending') + '">Wallet pending</span>';
    }
    return '<div class="flex max-w-full flex-wrap items-center justify-end gap-x-2 gap-y-1" data-forum-tip-control="' + escapeHtml(postId) + '">' +
      '<label class="flex items-center gap-1 font-sans text-xs text-forum-text"><span>Tip</span><input class="h-8 w-20 rounded border border-forum-row-c bg-forum-panel px-2 text-right text-forum-heading" data-forum-tip-amount="' + escapeHtml(postId) + '" inputmode="numeric" min="1" step="1" type="number" value="' + String(defaultPostRewardSats) + '"><span>sats</span></label>' +
      '<button type="button" class="min-h-8 rounded border border-forum-payment bg-forum-panel px-3 py-1.5 font-sans text-xs font-bold text-forum-payment hover:bg-forum-post-link-hover-bg focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-forum-header" data-forum-tip-post-id="' + escapeHtml(postId) + '">Send tip</button>' +
      '<span class="font-sans text-xs text-forum-text">to ' + (post.author && actorProfileHref(post.author) ? actorNameHtml(post.author) : escapeHtml(recipient)) + '</span>' +
      '<span class="min-w-0 break-words font-sans text-xs text-forum-text">' + postRewardCaveat + '</span>' +
      '<span class="basis-full text-right text-xs text-forum-text" data-forum-tip-panel="' + escapeHtml(postId) + '"></span>' +
      '</div>';
  };
  const postControlLink = (href, label) =>
    '<a class="rounded border border-forum-row-c bg-forum-panel px-2 py-1 text-xs font-bold text-forum-link hover:border-forum-header hover:bg-forum-post-link-hover-bg hover:text-forum-link-hover" href="' + escapeHtml(href) + '">' + escapeHtml(label) + '</a>';
  const renderPostControls = post =>
    // Capped width so the grid's auto column cannot size to the
    // controls' unwrapped max-content and crush the title.
    '<div class="flex max-w-full flex-wrap items-center justify-end gap-2 sm:max-w-[24rem]">' +
      postTipStatsBadge(post) +
      renderTipControls(post) +
      '<button type="button" class="rounded border border-forum-row-c bg-forum-panel px-2 py-1 text-xs font-bold text-forum-link hover:border-forum-header hover:bg-forum-post-link-hover-bg hover:text-forum-link-hover" data-forum-copy-permalink="' + escapeHtml(postHref(post)) + '">Permalink</button>' +
      '</div>';
  const renderAuthorProfile = post => {
    const actor = post.author || {};
    const displayName = actorDisplayName(actor);
    const postCount = actor.postCount ?? actor.forumPostCount ?? post.authorPostCount;
    const joinedAt = actor.joinedAt || actor.firstSeenAt || post.authorFirstSeenAt;
    const readiness = post.tipRecipientReadiness?.tippingAvailable === true ? 'Wallet ready' : post.tipRecipientReadiness ? 'Wallet pending' : '';
    return '<aside class="grid content-start gap-2 border-b border-forum-row-c bg-forum-row-b p-3 text-sm text-forum-text md:border-b-0 md:border-r">' +
      '<div class="flex items-start gap-2">' +
      '<span class="flex size-10 shrink-0 items-center justify-center rounded bg-forum-header text-base font-bold text-white" aria-hidden="true">' + escapeHtml(actorInitial(actor)) + '</span>' +
      '<div class="min-w-0"><div class="break-words font-bold text-forum-link">' + (actorProfileHref(actor) ? '<a class="hover:text-forum-link-hover hover:underline" href="' + escapeHtml(actorProfileHref(actor)) + '">' + escapeHtml(displayName) + '</a>' : escapeHtml(displayName)) + '</div><div class="text-xs text-forum-text">' + escapeHtml(actorRole(actor)) + '</div></div>' +
      '</div>' +
      '<dl class="grid gap-1 text-xs">' +
      (postCount === undefined ? '' : '<div><dt class="inline font-bold">Posts:</dt> <dd class="inline">' + escapeHtml(postCount) + '</dd></div>') +
      (joinedAt ? '<div><dt class="inline font-bold">Joined:</dt> <dd class="inline">' + escapeHtml(friendlyTime(joinedAt)) + '</dd></div>' : '') +
      (readiness ? '<div><dt class="inline font-bold">Tips:</dt> <dd class="inline">' + escapeHtml(readiness) + '</dd></div>' : '') +
      '</dl>' +
      '</aside>';
  };
  const renderPostBody = post => {
    const postNumber = Number(post.postNumber || 0);
    const subject = post.subject || post.title || state.topic?.title || ('Post #' + postNumber);
    return '<div class="min-w-0 p-3">' +
      '<header class="grid gap-2 border-b border-forum-row-c pb-3 sm:grid-cols-[minmax(0,1fr)_auto]">' +
      '<div class="min-w-0"><h3 class="m-0 break-words text-base font-bold text-forum-link"><a class="hover:text-forum-link-hover hover:underline" href="' + escapeHtml(postHref(post)) + '">' + escapeHtml(subject) + '</a></h3>' +
      '<p class="m-0 mt-1 text-xs text-forum-text">Post #' + postNumber + ' &raquo; ' + friendlyTime(post.createdAt) + '</p></div>' +
      renderPostControls(post) +
      '</header>' +
      '<div data-forum-markdown class="mt-3 grid gap-3 break-words text-sm/6 text-forum-heading [overflow-wrap:anywhere]">' + renderMarkdown(post.bodyText || post.contentRef || '') + '</div>' +
      '</div>';
  };
  const renderTipResult = (post, result) => {
    if (result.receiptRef) {
      const receiptHref = '/forum/receipts/' + encodeURIComponent(result.receiptRef);
      return 'Payment recorded · <a class="text-forum-link underline underline-offset-4 hover:text-forum-link-hover" href="' + receiptHref + '">Receipt</a> · <a class="text-forum-link underline underline-offset-4 hover:text-forum-link-hover" href="' + escapeHtml(postHref(post)) + '">Post</a>';
    }
    if (result.writeDenial?.denialKind === 'recipient_not_ready') {
      return 'Recipient wallet pending.';
    }
    return 'Payment submitted.';
  };
  const handleTipClick = async button => {
    const postId = button.getAttribute('data-forum-tip-post-id') || '';
    const post = state.posts.find(item => item.postId === postId);
    if (!post) return;
    if (authMode !== 'LoggedIn') {
      setTipPanel(postId, 'login_required', '<a class="text-forum-link underline underline-offset-4 hover:text-forum-link-hover" href="' + escapeHtml(loginHref) + '">Log in with GitHub</a> to tip ' + escapeHtml(post.author?.displayName || 'creator') + '.');
      return;
    }
    button.disabled = true;
    const amountInput = Array.from(main.querySelectorAll('[data-forum-tip-amount]')).find(element =>
      element.getAttribute('data-forum-tip-amount') === postId
    );
    const rawAmount = Math.trunc(Number(amountInput?.value || defaultPostRewardSats));
    const amount = Number.isFinite(rawAmount) && rawAmount > 0 ? rawAmount : defaultPostRewardSats;
    setTipPanel(postId, 'sending', 'Sending ' + postRewardAmountLabel(amount) + '...');
    try {
      const result = await api('/api/forum/posts/' + encodeURIComponent(postId) + '/tips/ladder', {
        method: 'POST',
        headers: {
          'Idempotency-Key': tipIdempotencyKey(postId + ':' + String(amount)),
        },
        body: JSON.stringify({
          amountSat: amount,
        }),
      });
      setTipPanel(postId, result.receiptRef ? 'success' : 'failed', renderTipResult(post, result));
    } catch (error) {
      setTipPanel(postId, 'failed', 'Payment failed · ' + escapeHtml(error.message || error));
    } finally {
      button.disabled = false;
    }
  };
  main.addEventListener('click', event => {
    const target = event.target instanceof Element
      ? event.target.closest('[data-forum-tip-post-id]')
      : null;
    if (target instanceof HTMLButtonElement) {
      event.preventDefault();
      handleTipClick(target);
    }
  });

  const postRows = posts => posts.length === 0
    ? '<div class="border-t border-forum-row-c py-8 text-sm text-forum-text">No visible posts yet.</div>'
    : posts.map((post, index) => '<article id="' + escapeHtml(postAnchor(post)) + '" data-forum-post-id="' + escapeHtml(post.postId || '') + '" data-forum-post-number-anchor="' + escapeHtml(postNumberAnchor(post)) + '" tabindex="-1" class="scroll-mt-6 grid overflow-hidden border-t border-forum-row-c outline-none target:border-forum-payment target:bg-forum-post-link-hover-bg focus-visible:border-forum-header md:grid-cols-[12rem_minmax(0,1fr)] ' + rowClass(index) + '">' +
        renderAuthorProfile(post) + renderPostBody(post) + '</article>').join('');
  const topicSortToggle = topic => {
    const active = state.topicPostSortDirection === 'desc' ? 'desc' : 'asc';
    const item = (direction, label) => {
      const isActive = active === direction;
      return '<a class="${ghostButtonClass} ' + (isActive ? 'border-forum-header bg-forum-post-link-hover-bg text-forum-heading' : '') + '" href="' + topicSortHref(topic, direction) + '" aria-current="' + (isActive ? 'true' : 'false') + '">' + label + '</a>';
    };
    return '<div class="flex flex-wrap items-center gap-2" aria-label="Post order">' + item('asc', 'Oldest first') + item('desc', 'Newest first') + '</div>';
  };

  const renderIndex = data => {
    const forums = data.forums || [];
    main.innerHTML = '<nav class="${forumBreadcrumbClass}" aria-label="Forum breadcrumbs"><a class="font-bold text-forum-link hover:text-forum-link-hover" href="/forum">Board index</a></nav>' +
      '<section class="' + forumListClass + '"><div class="${forumHeaderClass} rounded-none">OpenAgents Forum</div>' +
      listHeader(forumGridClass, 'Forum', 'Topics', 'Posts', 'Last post') +
      forumRows(forums) +
      '</section>' +
      renderTipLeaderboards(state.tipLeaderboards);
  };
  const renderForum = (forum, topics) => {
    main.innerHTML = '<nav class="${forumBreadcrumbClass}" aria-label="Forum breadcrumbs"><a class="font-bold text-forum-link hover:text-forum-link-hover" href="/forum">Board index</a> &raquo; <span>' + escapeHtml(forum.title) + '</span></nav>' +
      actionBar('<div class="flex flex-wrap items-center gap-2">' + forumBadge(forum) + '<a class="${ghostButtonClass}" href="/forum">Board</a></div>' + pageSummary(topics.length, 'topic')) +
      '<section class="${panelClass} overflow-hidden"><div class="${forumHeaderClass} rounded-none">Forum</div><div class="flex flex-wrap items-start justify-between gap-3 p-4 sm:p-5">' +
      '<div><p class="${eyebrowClass}">Forum</p><h1 class="m-0 text-2xl font-bold text-forum-heading sm:text-3xl">' + escapeHtml(forum.title) + '</h1>' +
      '<p class="mt-2 ${mutedClass}">' + topicCountText(forum.topicCount) + ' · ' + postCountText(forum.postCount) + (forum.locked ? ' · Locked' : '') + '</p></div>' +
      '<div class="flex flex-wrap gap-2">' + forumBadge(forum) + '<a class="${ghostButtonClass}" href="/forum">Board</a></div></div>' +
      '<div class="px-4 pb-4 sm:px-5 sm:pb-5"><div class="' + forumListClass + '">' +
      listHeader(topicGridClass, 'Topics', 'Replies', 'Views', 'Last post') +
      topicRows(topics) +
      '</div></div></section>' +
      actionBar('<span></span>' + pageSummary(topics.length, 'topic'));
  };
  const renderTopic = (topic, posts) => {
    main.innerHTML = '<nav class="${forumBreadcrumbClass}" aria-label="Forum breadcrumbs"><a class="font-bold text-forum-link hover:text-forum-link-hover" href="/forum">Board index</a> &raquo; <a class="font-bold text-forum-link hover:text-forum-link-hover" href="' + forumHref({ slug: topic.forumId, forumId: topic.forumId }) + '">Forum</a> &raquo; <span>' + escapeHtml(topic.title) + '</span></nav>' +
      actionBar('<a class="${ghostButtonClass}" href="' + forumHref({ slug: topic.forumId, forumId: topic.forumId }) + '">Forum</a><div class="flex flex-wrap items-center gap-2">' + topicSortToggle(topic) + pageSummary(posts.length, 'post') + '</div>') +
      '<section class="${panelClass} overflow-hidden"><div class="${forumHeaderClass}">Topic</div><div class="flex flex-wrap items-start justify-between gap-3 p-4 sm:p-5">' +
      '<div><p class="${eyebrowClass}">Thread</p><h1 class="m-0 text-2xl font-bold text-forum-heading">' + escapeHtml(topic.title) + '</h1>' +
      '<p class="mt-2 ${mutedClass}">' + postCountText(topic.postCount) + '</p></div>' +
      '<a class="${ghostButtonClass}" href="' + forumHref({ slug: topic.forumId, forumId: topic.forumId }) + '">Forum</a></div>' +
      '<div class="px-4 pb-4 sm:px-5 sm:pb-5">' + postRows(posts) + '</div></section>' +
      actionBar(topicSortToggle(topic) + pageSummary(posts.length, 'post'));
    requestAnimationFrame(scrollPostAnchorIntoView);
  };
  const amountText = amount => {
    if (!amount) return 'Recorded payment';
    if (amount.asset === 'sats') return String(amount.amount) + ' sats of bitcoin';
    if (amount.asset === 'usd') return '$' + String(amount.amount / 100);
    return String(amount.amount) + ' credits';
  };
  const actionText = actionKind => String(actionKind || 'paid_action').replaceAll('_', ' ');
  const renderReceiptTarget = receipt => {
    const target = receipt?.target || {};
    const links = [];
    if (receipt?.targetPostPermalink) links.push('<a class="text-forum-link underline underline-offset-4 hover:text-forum-link-hover" href="' + escapeHtml(receipt.targetPostPermalink) + '">Post</a>');
    if (target && target.topicId) links.push('<a class="text-forum-link underline underline-offset-4 hover:text-forum-link-hover" href="/forum/t/' + encodeURIComponent(target.topicId) + '">Topic</a>');
    if (!receipt?.targetPostPermalink && target && target.topicId && target.postId) links.push('<a class="text-forum-link underline underline-offset-4 hover:text-forum-link-hover" href="/forum/t/' + encodeURIComponent(target.topicId) + '#post-' + encodeURIComponent(target.postId) + '">Post</a>');
    if (target && !target.topicId && target.postId) links.push('<a class="text-forum-link underline underline-offset-4 hover:text-forum-link-hover" href="/api/forum/posts/' + encodeURIComponent(target.postId) + '">Post API</a>');
    return links.length === 0 ? '<span class="text-forum-text">Forum payment</span>' : links.join(' · ');
  };
  const renderTipSettlement = settlement => {
    if (!settlement) return '';
    const settlementEvidenceLabel = settlement.creatorReceivedSpendableValue ? 'Recipient wallet payment confirmed' : 'Recipient wallet payment not confirmed';
    return '<div class="mt-4 rounded border border-forum-row-c bg-forum-row-a p-3">' +
      '<div class="font-sans text-xs text-forum-text">Tip settlement</div>' +
      '<div class="mt-1 text-sm font-bold text-forum-heading">' + escapeHtml(tipStateLabel(settlement.state)) + '</div>' +
      '<p class="m-0 mt-1 text-sm text-forum-text">' + escapeHtml(settlement.wording?.publicPage || 'Settlement state is pending.') + '</p>' +
      '<div class="mt-2 font-sans text-xs text-forum-text">' + escapeHtml(settlementEvidenceLabel) + '</div>' +
      '</div>';
  };
  const renderReceipt = receipt => {
    main.innerHTML = '<nav class="${forumBreadcrumbClass}" aria-label="Forum breadcrumbs"><a class="font-bold text-forum-link hover:text-forum-link-hover" href="/forum">Board index</a> &raquo; <span>Receipt</span></nav>' +
      '<section class="${panelClass} overflow-hidden"><div class="${forumHeaderClass}">Forum receipt</div><div class="flex flex-wrap items-start justify-between gap-3 border-b border-forum-row-c p-4 sm:p-5">' +
      '<div><p class="${eyebrowClass}">Forum receipt</p><h1 class="m-0 text-2xl font-bold text-forum-heading sm:text-3xl">' + escapeHtml(actionText(receipt.actionKind)) + '</h1>' +
      '<p class="mt-2 ${mutedClass}">' + escapeHtml(amountText(receipt.amount)) + ' · ' + friendlyTime(receipt.createdAt) + '</p></div>' +
      '<a class="${ghostButtonClass}" href="/forum">Board</a></div>' +
      '<div class="p-4 sm:p-5">' + renderTipSettlement(receipt.tipSettlement) +
      '<dl class="mt-4 grid gap-0 text-sm">' +
      '<div class="grid gap-2 border-t border-forum-row-c py-3 sm:grid-cols-[10rem_1fr]"><dt class="${eyebrowClass}">Receipt</dt><dd class="m-0 min-w-0 break-words text-forum-heading [overflow-wrap:anywhere]">' + escapeHtml(receipt.receiptRef || '') + '</dd></div>' +
      '<div class="grid gap-2 border-t border-forum-row-c py-3 sm:grid-cols-[10rem_1fr]"><dt class="${eyebrowClass}">Target</dt><dd class="m-0 text-forum-heading">' + renderReceiptTarget(receipt) + '</dd></div>' +
      '<div class="grid gap-2 border-t border-forum-row-c py-3 sm:grid-cols-[10rem_1fr]"><dt class="${eyebrowClass}">Recipient</dt><dd class="m-0 min-w-0 break-words text-forum-heading [overflow-wrap:anywhere]">' + escapeHtml(receipt.recipientActorRef || 'OpenAgents moderation pool') + '</dd></div>' +
      '</dl></div></section>';
  };
  const routePlan = () => {
    if (initial.kind === 'forum') {
      return {
        paths: [
          '/api/forum/forums/' + encodeURIComponent(initial.ref),
          '/api/forum/forums/' + encodeURIComponent(initial.ref) + '/topics',
        ],
        apply: results => {
          state.forum = results[0]; state.topics = results[1].topics || [];
          renderForum(state.forum, state.topics);
        },
      };
    }
    if (initial.kind === 'topic') {
      return {
        paths: [
          topicApiPath(initial.id),
          '/api/forum/launch-status',
        ],
        apply: results => {
          state.topic = results[0].topic; state.posts = results[0].posts || [];
          state.launchStatus = results[1];
          renderTopic(state.topic, state.posts);
        },
      };
    }
    if (initial.kind === 'receipt') {
      return {
        paths: ['/api/forum/receipts/' + encodeURIComponent(initial.ref)],
        apply: results => { state.receipt = results[0]; renderReceipt(state.receipt); },
      };
    }
    return {
      paths: ['/api/forum', '/api/forum/tip-leaderboards'],
      apply: results => {
        state.tipLeaderboards = results[1];
        renderIndex(results[0]);
      },
    };
  };
  const load = async () => {
    const plan = routePlan();
    const cached = plan.paths.map(cacheGet);
    let renderedFromCache = false;
    if (cached.every(entry => entry !== null)) {
      try { plan.apply(cached); renderedFromCache = true; } catch {}
    }
    try {
      const fresh = await Promise.all(plan.paths.map(path => api(path)));
      fresh.forEach((data, index) => cacheSet(plan.paths[index], data));
      plan.apply(fresh);
    } catch (error) {
      if (!renderedFromCache) {
        main.innerHTML = '<section class="${panelClass} p-5"><p class="${eyebrowClass}">Forum unavailable</p><p class="${mutedClass}">' + escapeHtml(error.message || error) + '</p></section>';
      }
    }
  };
  load();
})();`
}

export const view = <Message>(
  route: ForumRouteValue,
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()
  const loginHref = forumLoginHref(route)

  return h.div(
    [h.DataAttribute('forum-shell', ''), Ui.className<Message>(shellClass)],
    [
      PublicHeader.view(authState, 'forum', loginHref),
      h.main(
        [
          h.DataAttribute('forum-app', ''),
          Ui.className<Message>(containerClass),
        ],
        [
          h.div([h.DataAttribute('forum-login-error', '')], []),
          h.div(
            [
              h.DataAttribute('forum-main', ''),
              Ui.className<Message>('grid min-w-0 gap-4'),
            ],
            [
              // Cold-load skeleton: the forum shell with pulsing ghost
              // rows instead of a text message; the cached or fetched
              // content replaces this whole mount.
              h.div(
                [
                  Ui.className<Message>(forumBreadcrumbClass),
                  h.AriaLabel('Loading'),
                ],
                [
                  h.span(
                    [
                      Ui.className<Message>(
                        'block h-4 w-28 animate-pulse rounded bg-forum-row-c',
                      ),
                    ],
                    [],
                  ),
                ],
              ),
              h.section(
                [Ui.className<Message>(`${panelClass} overflow-hidden`)],
                [
                  h.div(
                    [Ui.className<Message>(`${forumHeaderClass} rounded-none`)],
                    [
                      h.span(
                        [
                          Ui.className<Message>(
                            'block h-4 w-40 animate-pulse rounded bg-white/30',
                          ),
                        ],
                        [],
                      ),
                    ],
                  ),
                  ...[0, 1, 2, 3, 4].map(index =>
                    h.div(
                      [
                        Ui.className<Message>(
                          'grid grid-cols-[2.25rem_minmax(0,1fr)_8rem] items-center gap-3 border-t border-forum-row-c px-3 py-4 ' +
                            (index % 2 === 0
                              ? 'bg-forum-row-a'
                              : 'bg-forum-row-b'),
                        ),
                      ],
                      [
                        h.span(
                          [
                            Ui.className<Message>(
                              'block size-9 animate-pulse rounded bg-forum-row-c',
                            ),
                          ],
                          [],
                        ),
                        h.span(
                          [Ui.className<Message>('grid gap-2')],
                          [
                            h.span(
                              [
                                Ui.className<Message>(
                                  'block h-3.5 w-2/5 animate-pulse rounded bg-forum-row-c',
                                ),
                              ],
                              [],
                            ),
                            h.span(
                              [
                                Ui.className<Message>(
                                  'block h-3 w-3/5 animate-pulse rounded bg-forum-row-c/70',
                                ),
                              ],
                              [],
                            ),
                          ],
                        ),
                        h.span(
                          [
                            Ui.className<Message>(
                              'block h-3 w-full animate-pulse rounded bg-forum-row-c/70',
                            ),
                          ],
                          [],
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ],
          ),
        ],
      ),
      h.script([], [forumScript(route, authState._tag)]),
    ],
  )
}

export const title = (route: ForumRouteValue): string =>
  route._tag === 'ForumForum'
    ? `${route.forumRef} Forum - OpenAgents`
    : route._tag === 'ForumTopic'
      ? `${route.topicId.slice(0, 8)} Topic - OpenAgents`
      : route._tag === 'ForumReceipt'
        ? 'Forum Receipt - OpenAgents'
        : 'Forum - OpenAgents'
