import Constants from 'expo-constants';

interface Environment {
  releaseChannel: string;
  apiUrl: string;
  wsUrl: string;
  apiKey: string;
  debug: boolean;
}

// Default development environment
const defaultEnv: Environment = {
  releaseChannel: 'development',
  apiUrl: 'http://localhost:3000',
  wsUrl: 'ws://localhost:3000',
  apiKey: process.env.API_KEY || 'development_key',
  debug: true
};

// Get environment from Expo config
const env: Environment = {
  releaseChannel: Constants.expoConfig?.extra?.releaseChannel || defaultEnv.releaseChannel,
  apiUrl: Constants.expoConfig?.extra?.apiUrl || defaultEnv.apiUrl,
  wsUrl: Constants.expoConfig?.extra?.wsUrl || defaultEnv.wsUrl,
  apiKey: Constants.expoConfig?.extra?.apiKey || defaultEnv.apiKey,
  debug: Constants.expoConfig?.extra?.debug ?? defaultEnv.debug
};

export default env;
