import { SITE_DESCRIPTION, SITE_TITLE } from '@/consts';

export function buildHead(options?: {
  title?: string;
  description?: string;
  image?: string;
}) {
  const title = options?.title ?? SITE_TITLE;
  const description = options?.description ?? SITE_DESCRIPTION;
  const image = options?.image;

  return {
    meta: [
      { title },
      { name: 'description', content: description },
      { property: 'og:title', content: title },
      { property: 'og:description', content: description },
      {
        property: 'twitter:card',
        content: image ? 'summary_large_image' : 'summary',
      },
      { property: 'twitter:title', content: title },
      { property: 'twitter:description', content: description },
      ...(image
        ? [
            { property: 'og:image', content: image },
            { property: 'twitter:image', content: image },
          ]
        : []),
    ],
  };
}
