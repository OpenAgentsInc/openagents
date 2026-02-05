import type { HTMLAttributes, ReactNode } from 'react';
import clsx from 'clsx';
import { Animator } from '@arwes/react-animator';
import { Text, type TextProps } from '@arwes/react-text';

type HatcheryTextProps<E extends HTMLElement> = HTMLAttributes<E> &
  Omit<TextProps<E>, 'as' | 'children'> & {
    children: ReactNode;
  };

export function HatcheryH1(props: HatcheryTextProps<HTMLHeadingElement>) {
  const { className, children, ...rest } = props;
  return (
    <Animator>
      <Text
        {...rest}
        as="h1"
        manager="decipher"
        fixed={false}
        className={clsx('font-semibold text-2xl text-foreground', className)}
        contentClassName="text-foreground"
        contentStyle={{ color: 'hsl(0, 0%, 95%)' }}
        hideOnExited={false}
        hideOnEntered={false}
      >
        {children}
      </Text>
    </Animator>
  );
}

export function HatcheryH2(props: HatcheryTextProps<HTMLHeadingElement>) {
  const { className, children, ...rest } = props;
  return (
    <Animator>
      <Text
        {...rest}
        as="h2"
        manager="decipher"
        fixed={false}
        className={clsx('font-medium text-lg text-foreground', className)}
        contentClassName="text-foreground"
        contentStyle={{ color: 'hsl(0, 0%, 90%)' }}
        hideOnExited={false}
        hideOnEntered={false}
      >
        {children}
      </Text>
    </Animator>
  );
}

export function HatcheryP(props: HatcheryTextProps<HTMLParagraphElement>) {
  const { className, children, ...rest } = props;
  return (
    <Animator>
      <Text
        {...rest}
        as="p"
        manager="decipher"
        fixed={false}
        className={clsx('text-muted-foreground text-sm', className)}
        contentClassName="text-muted-foreground"
        contentStyle={{ color: 'hsl(0, 0%, 70%)' }}
        hideOnExited={false}
        hideOnEntered={false}
      >
        {children}
      </Text>
    </Animator>
  );
}
