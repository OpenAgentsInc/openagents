/**
 * Centralized platform detection utilities
 */

export interface PlatformInfo {
  isReactNative: boolean;
  isMobile: boolean;
  isDesktop: boolean;
  isWeb: boolean;
}

/**
 * Detects if the current environment is React Native
 */
export const isReactNative = (): boolean => {
  return (
    typeof window !== 'undefined' && 
    window.navigator?.product === 'ReactNative'
  );
};

/**
 * Detects if the current environment is a web browser
 */
export const isWeb = (): boolean => {
  return (
    typeof window !== 'undefined' && 
    typeof document !== 'undefined' &&
    !isReactNative()
  );
};

/**
 * Detects if the current environment is desktop (web or desktop app)
 */
export const isDesktop = (): boolean => {
  return isWeb() || (typeof window !== 'undefined' && !isReactNative());
};

/**
 * Detects if the current environment is mobile
 */
export const isMobile = (): boolean => {
  return isReactNative();
};

/**
 * Gets comprehensive platform information
 */
export const getPlatformInfo = (): PlatformInfo => {
  const reactNative = isReactNative();
  const web = isWeb();
  const mobile = reactNative;
  const desktop = !mobile;

  return {
    isReactNative: reactNative,
    isMobile: mobile,
    isDesktop: desktop,
    isWeb: web,
  };
};

/**
 * Gets a platform identifier string
 */
export const getPlatformId = (): 'mobile' | 'desktop' | 'web' => {
  if (isReactNative()) return 'mobile';
  if (isWeb()) return 'web';
  return 'desktop';
};