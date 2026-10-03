#!/usr/bin/env python3
"""Connect the executor's loopback HTTP listener to the provider Unix socket."""
import argparse
import selectors
import socket
import socketserver


class Bridge(socketserver.BaseRequestHandler):
    def handle(self):
        upstream=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)
        upstream.settimeout(300)
        upstream.connect(self.server.provider_socket)
        self.request.settimeout(300)
        with upstream, selectors.DefaultSelector() as selected:
            selected.register(self.request,selectors.EVENT_READ,upstream)
            selected.register(upstream,selectors.EVENT_READ,self.request)
            while True:
                ready=selected.select(300)
                if not ready:
                    return
                for key,_ in ready:
                    data=key.fileobj.recv(65536)
                    if not data:
                        return
                    key.data.sendall(data)


class Server(socketserver.ThreadingMixIn,socketserver.TCPServer):
    daemon_threads=True
    allow_reuse_address=False


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--socket',required=True)
    parser.add_argument('--port',type=int,default=18080)
    args=parser.parse_args()
    with Server(('127.0.0.1',args.port),Bridge) as server:
        server.provider_socket=args.socket
        server.serve_forever()


if __name__=='__main__':
    main()
