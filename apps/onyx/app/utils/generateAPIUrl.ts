import Constants from 'expo-constants';

const BASE_URL = "https://chat.openagents.com"

export const generateAPIUrl = (relativePath: string) => {
  const path = relativePath.startsWith('/') ? relativePath : `/${relativePath}`;

  // In development, try to use experienceUrl if available
  if (process.env.NODE_ENV === 'development' && Constants.experienceUrl) {
    const origin = Constants.experienceUrl.replace('exp://', 'http://');
    return origin.concat(path);
  }

  // Fallback to BASE_URL for production or when experienceUrl is not available
  return BASE_URL.concat(path);
};
