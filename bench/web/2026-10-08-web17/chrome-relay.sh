#!/bin/sh
exec "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --proxy-server=http://127.0.0.1:47999 "--proxy-bypass-list=<-loopback>" "$@"
