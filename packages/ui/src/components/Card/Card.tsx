import React from 'react';
import { View, ViewStyle } from 'react-native';

interface CardProps {
  children: React.ReactNode;
  style?: ViewStyle;
  padding?: 'none' | 'small' | 'medium' | 'large';
}

export function Card({ children, style, padding = 'medium' }: CardProps) {
  const getPadding = () => {
    switch (padding) {
      case 'small':
        return 8;
      case 'large':
        return 24;
      case 'none':
        return 0;
      default:
        return 16;
    }
  };

  return (
    <View
      style={[
        {
          backgroundColor: '#1A1A1A',
          borderRadius: 8,
          padding: getPadding(),
          shadowColor: '#000',
          shadowOffset: { width: 0, height: 2 },
          shadowOpacity: 0.25,
          shadowRadius: 3.84,
          elevation: 5,
        },
        style,
      ]}
    >
      {children}
    </View>
  );
}
