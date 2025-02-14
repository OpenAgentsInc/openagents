import asyncio
import websockets
import json

async def test_websocket():
    uri = "ws://localhost:8000/ws/solver"
    async with websockets.connect(uri) as websocket:
        print("Connected to WebSocket server")

        # Send initial message
        message = {
            "type": "start",
            "repo_url": "https://github.com/example/test",
            "issue_number": 1
        }
        await websocket.send(json.dumps(message))
        print(f"Sent message: {message}")

        # Listen for responses
        while True:
            try:
                response = await websocket.recv()
                print(f"Received: {response}")
            except websockets.exceptions.ConnectionClosed:
                print("Connection closed")
                break

if __name__ == "__main__":
    asyncio.run(test_websocket())
