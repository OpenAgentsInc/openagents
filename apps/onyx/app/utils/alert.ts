import { Alert } from "react-native"

// import * as log from './log';
const log = {
  error: console.error,
}

export function info(title, message) {
  Alert.alert(title, message);
}

export function error({ title, message, err }) {
  Alert.alert(title, message || err.message);
  if (err) {
    log.error(err);
  }
}

export function warn({ title, message, onOk, okText, err }) {
  Alert.alert(
    title,
    message || err.message,
    [
      {
        text: okText || 'OK',
        onPress: () => onOk(),
      },
    ],
    {
      cancelable: false,
    },
  );
  if (err) {
    log.error(err);
  }
}

export function confirm({ title, message, onOk, okText, destructive }) {
  Alert.alert(
    title,
    message,
    [
      {
        text: 'Cancel',
        style: 'cancel',
      },
      {
        text: okText || 'OK',
        style: destructive ? 'destructive' : 'default',
        onPress: () => onOk(),
      },
    ],
    {
      cancelable: true,
    },
  );
}
