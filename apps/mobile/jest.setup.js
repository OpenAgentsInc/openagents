import "@testing-library/jest-native/extend-expect";

// Mock AsyncStorage
jest.mock("@react-native-async-storage/async-storage", () =>
  require("@react-native-async-storage/async-storage/jest/async-storage-mock")
);

// Mock react-native-device-info
jest.mock("react-native-device-info", () => ({
  getUniqueId: jest.fn(() => "test-device-id"),
  getDeviceId: jest.fn(() => "test-device-id"),
  getSystemName: jest.fn(() => "iOS"),
  getSystemVersion: jest.fn(() => "14.0"),
}));

// Mock Expo modules
jest.mock("expo-font", () => ({
  loadAsync: jest.fn(),
  isLoaded: jest.fn(() => true),
}));

jest.mock("expo-secure-store", () => ({
  getItemAsync: jest.fn(),
  setItemAsync: jest.fn(),
  deleteItemAsync: jest.fn(),
}));

// Mock react-native modules
jest.mock("react-native/Libraries/Animated/NativeAnimatedHelper");

// Silence console warnings in tests
const originalWarn = console.warn;
beforeAll(() => {
  console.warn = (...args) => {
    if (
      args[0]?.includes?.("Warning: React.createElement") ||
      args[0]?.includes?.("Warning: Failed prop type")
    ) {
      return;
    }
    originalWarn.apply(console, args);
  };
});

afterAll(() => {
  console.warn = originalWarn;
});