import React, { createContext, useContext, useState, useCallback, useEffect, useRef } from 'react';
import {
  View,
  Text,
  TouchableOpacity,
  SafeAreaView,
  Animated,
  ViewStyle,
  TextStyle,
} from 'react-native';
import { ToastProps, ToastProviderProps, ToastOptions, ToastContextType } from './Toast.types';

// Create a unique ID for each toast
const generateId = () => Math.random().toString(36).substring(2, 9);

// Create context
const ToastContext = createContext<ToastContextType | undefined>(undefined);

// Toast component
export function Toast({
  visible,
  message,
  type = 'info',
  duration = 3000,
  onClose,
}: ToastProps) {
  const opacity = React.useRef(new Animated.Value(0)).current;

  useEffect(() => {
    if (visible) {
      Animated.sequence([
        Animated.timing(opacity, {
          toValue: 1,
          duration: 300,
          useNativeDriver: true,
        }),
        Animated.delay(duration),
        Animated.timing(opacity, {
          toValue: 0,
          duration: 300,
          useNativeDriver: true,
        }),
      ]).start(() => {
        onClose?.();
      });
    }
  }, [visible, duration, opacity, onClose]);

  if (!visible) return null;

  const backgroundColor = type === 'success' ? '#4CAF50' : type === 'error' ? '#F44336' : '#2196F3';

  return (
    <SafeAreaView style={$container}>
      <Animated.View
        style={[
          $toast,
          {
            opacity,
            backgroundColor,
          },
        ]}
      >
        <Text style={$message}>{message}</Text>
        <TouchableOpacity onPress={onClose} style={$closeButton}>
          <Text style={$closeText}>×</Text>
        </TouchableOpacity>
      </Animated.View>
    </SafeAreaView>
  );
}

// Toast provider
export const ToastProvider = ({ children }: ToastProviderProps) => {
  const [toasts, setToasts] = useState<Map<string, ToastOptions & { id: string }>>(new Map());

  const removeToast = useCallback((id: string) => {
    setToasts(prev => {
      const newMap = new Map(prev);
      newMap.delete(id);
      return newMap;
    });
  }, []);

  const show = useCallback((options: ToastOptions) => {
    const id = generateId();
    setToasts(prev => {
      const newMap = new Map(prev);
      newMap.set(id, { ...options, id });
      return newMap;
    });

    // Auto-close if duration is provided
    if (options.duration && options.duration > 0) {
      setTimeout(() => removeToast(id), options.duration);
    }

    return id;
  }, [removeToast]);

  const update = useCallback((id: string, options: ToastOptions) => {
    setToasts(prev => {
      if (!prev.has(id)) return prev;

      const newMap = new Map(prev);
      newMap.set(id, { ...prev.get(id), ...options, id });
      return newMap;
    });
  }, []);

  const close = useCallback((id: string) => {
    removeToast(id);
  }, [removeToast]);

  const closeAll = useCallback(() => {
    setToasts(new Map());
  }, []);

  const contextValue = {
    show,
    update,
    close,
    closeAll,
  };

  return (
    <ToastContext.Provider value={contextValue}>
      {children as React.ReactNode}
      <SafeAreaView style={$container} pointerEvents="box-none">
        {Array.from(toasts.values()).map((toast) => (
          <Toast
            key={toast.id}
            message={toast.message}
            type={toast.type}
            duration={toast.duration}
            onClose={() => removeToast(toast.id)}
          />
        ))}
      </SafeAreaView>
    </ToastContext.Provider>
  );
};

// Custom hook to use the toast context
export const useToast = () => {
  const context = useContext(ToastContext);
  if (context === undefined) {
    throw new Error('useToast must be used within a ToastProvider');
  }
  return context;
};

// Custom hook for interval
export const useInterval = (callback: () => void, delay: number | null) => {
  const savedCallback = useRef<() => void>(() => { });

  useEffect(() => {
    savedCallback.current = callback;
  }, [callback]);

  useEffect(() => {
    function tick() {
      if (savedCallback.current) savedCallback.current();
    }

    if (delay !== null) {
      const id = setInterval(tick, delay);
      return () => clearInterval(id);
    }

    return undefined;
  }, [delay]);
};

const $container: ViewStyle = {
  position: 'absolute',
  top: 0,
  left: 0,
  right: 0,
  zIndex: 999,
};

const $toast: ViewStyle = {
  margin: 16,
  padding: 16,
  borderRadius: 8,
  flexDirection: 'row',
  alignItems: 'center',
  justifyContent: 'space-between',
  shadowColor: '#000',
  shadowOffset: { width: 0, height: 2 },
  shadowOpacity: 0.25,
  shadowRadius: 3.84,
  elevation: 5,
};

const $message: TextStyle = {
  color: '#fff',
  fontSize: 16,
  flex: 1,
};

const $closeButton: ViewStyle = {
  marginLeft: 16,
};

const $closeText: TextStyle = {
  color: '#fff',
  fontSize: 24,
  lineHeight: 24,
};
