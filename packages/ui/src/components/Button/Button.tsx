import React from 'react';
import { View, Text, TouchableOpacity, ActivityIndicator } from 'react-native';

interface ButtonProps {
  label?: string;
  onPress?: () => void;
  variant?: 'primary' | 'secondary';
  size?: 'small' | 'medium' | 'large';
  disabled?: boolean;
  loading?: boolean;
  style?: any;
  leftIcon?: string;
  renderIcon?: (icon: string) => React.ReactNode;
}

export function Button({
  label,
  onPress,
  variant = 'primary',
  size = 'medium',
  disabled = false,
  loading = false,
  style,
  leftIcon,
  renderIcon,
}: ButtonProps) {
  return (
    <TouchableOpacity
      onPress={onPress}
      disabled={disabled || loading}
      style={[
        {
          flexDirection: 'row',
          alignItems: 'center',
          justifyContent: 'center',
          padding: size === 'small' ? 8 : size === 'medium' ? 12 : 16,
          borderRadius: 8,
          backgroundColor: variant === 'primary' ? '#007AFF' : '#333333',
          opacity: disabled ? 0.5 : 1,
        },
        style,
      ]}
    >
      {loading ? (
        <ActivityIndicator color="#fff" />
      ) : (
        <>
          {leftIcon && renderIcon && (
            <View style={{ marginRight: 8 }}>
              {renderIcon(leftIcon)}
            </View>
          )}
          {label && (
            <Text style={{ color: '#fff', fontSize: 16 }}>
              {label}
            </Text>
          )}
        </>
      )}
    </TouchableOpacity>
  );
}
