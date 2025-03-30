import React from 'react';
import { CardProps } from './Card.types';
import { View } from '@openagents/core';
import { styles, getCardPadding } from './Card.styles';

export const Card = ({
  children,
  padding = 'medium',
  borderWidth = 1,
  style,
  ...rest
}: CardProps) => {
  const paddingValue = getCardPadding(padding);

  return (
    <View
      style={[
        styles.card,
        {
          padding: paddingValue,
          borderWidth,
        },
        style,
      ]}
      {...rest}
    >
      {children as JSX.Element}
    </View>
  );
};

export default Card;
