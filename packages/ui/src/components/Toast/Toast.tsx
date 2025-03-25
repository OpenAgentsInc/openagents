import React, { createContext, useContext, useState, useCallback, useEffect, useRef } from 'react';
import { View, Text, Animated, TouchableOpacity, SafeAreaView } from 'react-native';
import { Ionicons } from '@expo/vector-icons';
import { ToastProps, ToastProviderProps, ToastOptions, ToastContextType } from './Toast.types';
import { styles, getVariantStyles } from './Toast.styles';

// Create a unique ID for each toast
const generateId = () => Math.random().toString(36).substring(2, 9);

// Create context
const ToastContext = createContext<ToastContextType | undefined>(undefined);

// Toast component
export const Toast = ({
  title,
  message,
  variant = 'default',
  icon,
  action,
  style,
  onClose,
}: ToastProps) => {
  const animation = useRef(new Animated.Value(0)).current;

  useEffect(() => {
    Animated.timing(animation, {
      toValue: 1,
      duration: 300,
      useNativeDriver: true,
    }).start();

    return () => {
      animation.setValue(0);
    };
  }, []);

  const handleClose = () => {
    Animated.timing(animation, {
      toValue: 0,
      duration: 300,
      useNativeDriver: true,
    }).start(() => {
      if (onClose) onClose();
    });
  };

  const variantStyles = getVariantStyles(variant);

  return (
    <Animated.View
      style={[
        styles.toast,
        variantStyles,
        style,
        {
          opacity: animation,
          transform: [
            {
              translateY: animation.interpolate({
                inputRange: [0, 1],
                outputRange: [-20, 0],
              }),
            },
          ],
        },
      ]}
    >
      {icon && <View>{icon}</View>}
      
      <View style={styles.contentContainer}>
        {title && <Text style={styles.title}>{title}</Text>}
        <Text style={styles.message}>{message}</Text>
      </View>
      
      {action && <View style={styles.actionContainer}>{action}</View>}
      
      <TouchableOpacity style={styles.closeButton} onPress={handleClose}>
        <Ionicons name="close" size={18} style={styles.closeIcon} />
      </TouchableOpacity>
    </Animated.View>
  );
};

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
      {children}
      <SafeAreaView style={styles.container} pointerEvents="box-none">
        {Array.from(toasts.values()).map((toast) => (
          <Toast
            key={toast.id}
            message={toast.message}
            title={toast.title}
            variant={toast.variant}
            icon={toast.icon}
            action={toast.action}
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
  const savedCallback = useRef<() => void>();

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
