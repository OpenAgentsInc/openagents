import type { ImgHTMLAttributes } from 'react';

/**
 * Simple image component (Arwes-style: minimal wrapper around img).
 * Use for logos, mascots, and decorative images with consistent props.
 */
export interface ImageProps extends ImgHTMLAttributes<HTMLImageElement> {
  src: string;
  alt: string;
}

export function Image(props: ImageProps): JSX.Element {
  const { src, alt, ...rest } = props;
  return <img src={src} alt={alt} decoding="async" {...rest} />;
}
