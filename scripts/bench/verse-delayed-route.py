#!/usr/bin/env python3
"""Forward an opaque loopback TLS stream with bounded directional delay."""
import argparse
import asyncio
import json
import pathlib
import signal
import time


async def scheduled_copy(reader, writer, direction, args, stats):
    queue = asyncio.Queue(maxsize=32)

    async def read_chunks():
        count = 0
        try:
            while chunk := await reader.read(65536):
                jitter = (count % 3) * args.jitter_ms / 2
                due = time.monotonic() + (args.delay_ms + jitter) / 1000
                await queue.put((due, chunk))
                stats["delay_queue_peak"] = max(stats["delay_queue_peak"], queue.qsize())
                count += 1
        except Exception as error:
            await queue.put(error)
            return
        await queue.put(None)

    reading = asyncio.create_task(read_chunks())
    try:
        while (item := await queue.get()) is not None:
            if isinstance(item, Exception):
                raise item
            due, chunk = item
            await asyncio.sleep(max(0, due - time.monotonic()))
            writer.write(chunk)
            await writer.drain()
            stats[direction + "_bytes"] += len(chunk)
            stats["forwarded_chunks"] += 1
    finally:
        reading.cancel()
        await asyncio.gather(reading, return_exceptions=True)


async def run(args):
    stats = {"schema": "verse.delayed-route.v2", "delay_ms": args.delay_ms,
             "jitter_ms": args.jitter_ms, "delay_mode": args.delay_mode,
             "delay_queue_capacity": 32, "delay_queue_peak": 0, "connections": 0,
             "upstream_bytes": 0, "downstream_bytes": 0,
             "forwarded_chunks": 0, "errors": 0, "refused_connections": 0,
             "error_details": [], "omitted_error_details": 0,
             "limits": ["Delay applies to each TCP read chunk, not decrypted game messages.",
                        "Scheduled mode bounds queued chunks and preserves order without a per-chunk throughput cap.",
                        "Serial mode adds delay after each read and also limits chunk throughput.",
                        "TCP preserves byte order; this fixture does not simulate packet loss."]}
    stopped = asyncio.Event()
    tasks = set()
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, stopped.set)

    def record_error(error, direction):
        stats["errors"] += 1
        if len(stats["error_details"]) < 128:
            stats["error_details"].append({"elapsed_seconds": time.monotonic() - started,
                                           "direction": direction,
                                           "type": type(error).__name__,
                                           "errno": getattr(error, "errno", None)})
        else:
            stats["omitted_error_details"] += 1

    async def copy(reader, writer, direction):
        if args.delay_mode == "scheduled":
            return await scheduled_copy(reader, writer, direction, args, stats)
        count = 0
        while chunk := await reader.read(65536):
            jitter = (count % 3) * args.jitter_ms / 2
            await asyncio.sleep((args.delay_ms + jitter) / 1000)
            writer.write(chunk)
            await writer.drain()
            stats[direction + "_bytes"] += len(chunk)
            stats["forwarded_chunks"] += 1
            count += 1

    async def connect(reader, writer):
        if len(tasks) >= args.connections:
            stats["refused_connections"] += 1
            writer.close()
            return
        task = asyncio.current_task()
        tasks.add(task)
        upstream = None
        stats["connections"] += 1
        direction = "connect"
        try:
            remote, upstream = await asyncio.wait_for(
                asyncio.open_connection("127.0.0.1", args.destination_port), 5)
            pipes = [asyncio.create_task(copy(reader, upstream, "upstream")),
                     asyncio.create_task(copy(remote, writer, "downstream"))]
            direction = "forward"
            try:
                done, pending = await asyncio.wait(pipes, return_when=asyncio.FIRST_COMPLETED)
                for finished in done:
                    direction = "upstream" if finished is pipes[0] else "downstream"
                    finished.result()
            finally:
                for pipe in pipes:
                    pipe.cancel()
                await asyncio.gather(*pipes, return_exceptions=True)
        except (OSError, asyncio.TimeoutError) as error:
            record_error(error, direction)
        finally:
            writer.close()
            if upstream:
                upstream.close()
            tasks.discard(task)

    started = time.monotonic()
    server = await asyncio.start_server(connect, "127.0.0.1", args.listen_port, limit=65536)
    address = server.sockets[0].getsockname()
    pathlib.Path(args.ready).write_text(json.dumps({"address": f"127.0.0.1:{address[1]}"}))
    try:
        await asyncio.wait_for(stopped.wait(), args.seconds)
    except asyncio.TimeoutError:
        pass
    finally:
        server.close()
        await server.wait_closed()
        for task in tuple(tasks):
            task.cancel()
        await asyncio.gather(*tuple(tasks), return_exceptions=True)
        stats["wall_seconds"] = time.monotonic() - started
        pathlib.Path(args.receipt).write_text(json.dumps(stats, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--connections", type=int, default=3)
    parser.add_argument("--destination-port", type=int, required=True)
    parser.add_argument("--listen-port", type=int, default=0)
    parser.add_argument("--delay-mode", choices=["scheduled", "serial"], default="scheduled")
    parser.add_argument("--delay-ms", type=int, default=40)
    parser.add_argument("--jitter-ms", type=int, default=20)
    parser.add_argument("--seconds", type=int, default=180)
    parser.add_argument("--ready", required=True)
    parser.add_argument("--receipt", required=True)
    args = parser.parse_args()
    if not (1 <= args.connections <= 32 and 1 <= args.destination_port <= 65535 and 0 <= args.listen_port <= 65535
            and 0 <= args.delay_ms <= 250 and 0 <= args.jitter_ms <= 100
            and 1 <= args.seconds <= 300):
        parser.error("Ports, delay, jitter, or duration exceed fixture bounds")
    asyncio.run(run(args))
