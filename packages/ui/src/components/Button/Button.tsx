import React from 'react';
import { ButtonProps } from './Button.types';
import { View, Text, TouchableOpacity, ActivityIndicator } from '@openagents/core';
import { getButtonStyles, getButtonHeight, getTextStyle, getTextSize, styles, COLORS } from './Button.styles';

export const Button = ({
  label,
  variant = 'primary',
  size = 'medium',
  loading = false,
  disabled = false,
  style,
  onPress,
  leftIcon,
  rightIcon,
  renderIcon,
  ...rest
}: ButtonProps) => {
  const buttonStyles = getButtonStyles(variant, disabled);
  const height = getButtonHeight(size);
  const textStyles = getTextStyle(variant, disabled);
  const fontSize = getTextSize(size);

  return (
    <TouchableOpacity
      style={[
        buttonStyles,
        { height },
        style,
      ]}
      disabled={disabled || loading}
      onPress={onPress}
      activeOpacity={0.8}
      {...rest}
    >
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'center' }}>
        {loading && (
          <ActivityIndicator
            size="small"
            color={variant === 'tertiary' ? COLORS.black : COLORS.white}
            style={styles.activityIndicator}
          />
        )}
        {!loading && leftIcon && renderIcon && (
          <View style={{ marginRight: 8 }}>
            {renderIcon(leftIcon) as JSX.Element}
          </View>
        )}
        <Text style={[textStyles, { fontSize }]}>{label}</Text>
        {!loading && rightIcon && renderIcon && (
          <View style={{ marginLeft: 8 }}>
            {renderIcon(rightIcon) as JSX.Element}
          </View>
        )}
      </View>
    </TouchableOpacity>
  );
};

export default Button;
