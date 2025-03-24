import { Text, TouchableOpacity, ActivityIndicator, View } from 'react-native';
import { ButtonProps } from './Button.types';
import { getButtonStyles, getButtonHeight, getTextStyle, getTextSize, styles, COLORS } from './Button.styles';

export const Button = ({
  label,
  variant = 'primary',
  size = 'medium',
  loading = false,
  disabled = false,
  style,
  onPress,
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
      {...rest}
    >
      <View style={{ flexDirection: 'row', alignItems: 'center' }}>
        {loading && (
          <ActivityIndicator
            size="small"
            color={variant === 'tertiary' ? COLORS.black : COLORS.white}
            style={styles.activityIndicator}
          />
        )}
        <Text style={[textStyles, { fontSize }]}>{label}</Text>
      </View>
    </TouchableOpacity>
  );
};

export default Button;
