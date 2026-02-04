import { describe, expect, it } from 'vitest';
import { buildHead } from './seo';
import { SITE_DESCRIPTION, SITE_TITLE } from '@/consts';

describe('seo.buildHead', () => {
  it('uses defaults when no options provided', () => {
    const head = buildHead();
    const meta = head.meta;
    expect(meta).toEqual(
      expect.arrayContaining([
        { title: SITE_TITLE },
        { name: 'description', content: SITE_DESCRIPTION },
      ]),
    );
  });

  it('adds image meta when image is provided', () => {
    const head = buildHead({
      title: 'Custom',
      description: 'Desc',
      image: 'https://example.com/image.png',
    });
    expect(head.meta).toEqual(
      expect.arrayContaining([
        { property: 'og:image', content: 'https://example.com/image.png' },
        { property: 'twitter:image', content: 'https://example.com/image.png' },
        { property: 'twitter:card', content: 'summary_large_image' },
      ]),
    );
  });
});
