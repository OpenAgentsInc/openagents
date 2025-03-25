import React from 'react';
import { View } from 'react-native';
import { CardProps } from './Card.types';
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
      {children}
    </View>
  );
};

export default Card;
