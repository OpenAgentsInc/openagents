#!/usr/bin/env python3
"""Check ordered delayed delivery, bounded buffering, and failure cleanup."""
import asyncio
import importlib.util
import pathlib
import types
import unittest

spec = importlib.util.spec_from_file_location("route", pathlib.Path(__file__).with_name("verse-delayed-route.py"))
route = importlib.util.module_from_spec(spec)
spec.loader.exec_module(route)


class Reader:
    def __init__(self, chunks, failure=None):
        self.chunks = iter(chunks)
        self.calls = 0
        self.failure = failure

    async def read(self, limit):
        self.calls += 1
        chunk = next(self.chunks, None)
        if chunk is None and self.failure:
            raise self.failure
        return chunk or b""


class Writer:
    def __init__(self, reader, failure=None):
        self.reader = reader
        self.failure = failure
        self.chunks = []
        self.reads_at_first_write = None

    def write(self, chunk):
        if self.reads_at_first_write is None:
            self.reads_at_first_write = self.reader.calls
        self.chunks.append(chunk)

    async def drain(self):
        if self.failure:
            raise self.failure
        await asyncio.sleep(0)


class DelayTests(unittest.IsolatedAsyncioTestCase):
    async def copy(self, reader, writer):
        stats = {"delay_queue_peak": 0, "upstream_bytes": 0, "forwarded_chunks": 0}
        args = types.SimpleNamespace(delay_ms=30, jitter_ms=20)
        await asyncio.wait_for(route.scheduled_copy(reader, writer, "upstream", args, stats), 2)
        return stats

    async def test_delay_does_not_serialize_chunk_reads(self):
        reader = Reader([b"one", b"two", b"three"])
        writer = Writer(reader)
        stats = await self.copy(reader, writer)
        self.assertEqual(writer.chunks, [b"one", b"two", b"three"])
        self.assertEqual(writer.reads_at_first_write, 4)
        self.assertEqual(stats["upstream_bytes"], 11)
        self.assertEqual(stats["forwarded_chunks"], 3)

    async def test_buffer_is_bounded_and_preserves_byte_order(self):
        chunks = [bytes([i]) for i in range(100)]
        reader = Reader(chunks)
        writer = Writer(reader)
        stats = await self.copy(reader, writer)
        self.assertEqual(writer.chunks, chunks)
        self.assertEqual(stats["delay_queue_peak"], 32)
        self.assertLessEqual(writer.reads_at_first_write, 34)

    async def test_read_failure_follows_already_received_bytes(self):
        reader = Reader([b"prior"], OSError("Fixture read failure"))
        writer = Writer(reader)
        with self.assertRaises(OSError):
            await self.copy(reader, writer)
        self.assertEqual(writer.chunks, [b"prior"])

    async def test_write_failure_cancels_a_blocked_reader(self):
        reader = Reader([b"x"] * 100)
        writer = Writer(reader, BrokenPipeError("Fixture write failure"))
        before = asyncio.all_tasks()
        with self.assertRaises(BrokenPipeError):
            await self.copy(reader, writer)
        self.assertEqual(asyncio.all_tasks(), before)


if __name__ == "__main__":
    unittest.main()
