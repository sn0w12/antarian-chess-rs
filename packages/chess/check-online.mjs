import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { connectOnline, loadEngine } from "./dist/index.js";

const here = path.dirname(fileURLToPath(import.meta.url));
const wasmBytes = fs.readFileSync(path.join(here, "dist/chess_wasm.wasm"));

class FakeSocket {
    static instances = [];

    constructor(url) {
        this.url = url;
        this.readyState = 0;
        this.OPEN = 1;
        this.sent = [];
        FakeSocket.instances.push(this);
    }

    open() {
        this.readyState = this.OPEN;
        this.onopen?.({});
    }

    send(data) {
        this.sent.push(JSON.parse(data));
    }

    close() {
        this.readyState = 3;
        this.onclose?.({});
    }

    receive(message) {
        this.onmessage?.({ data: JSON.stringify(message) });
    }
}

const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

const engine = await loadEngine({ wasmBytes });
const connecting = connectOnline({
    name: "web",
    engine,
    WebSocket: FakeSocket,
});
const socket = FakeSocket.instances.at(-1);
socket.open();
await tick();

const join = socket.sent.at(-1);
if (join?.type !== "Join" || join.data.name !== "web") {
    throw new Error("join message missing");
}

socket.receive({ type: "Joined", data: { player_id: "p1" } });
const client = await connecting;
if (client.playerId !== "p1") {
    throw new Error("player id missing");
}

socket.receive({ type: "MatchmakingStarted" });
if (client.status !== "matchmaking") {
    throw new Error("matchmaking status missing");
}

socket.receive({
    type: "MatchFound",
    data: { game_id: "g1", opponent_name: "bob", your_color: "white" },
});
if (
    client.status !== "playing" ||
    client.gameId !== "g1" ||
    client.color !== "white"
) {
    throw new Error("match found was not applied");
}

const move = client.play("b1", "c3");
if (!move) {
    throw new Error("online move rejected");
}
const sentMove = socket.sent.at(-1);
if (
    sentMove.type !== "MakeMove" ||
    sentMove.data.from !== 1 ||
    sentMove.data.to !== 18
) {
    throw new Error("move was not sent");
}
if (client.game.state().turn !== "black") {
    throw new Error("local board did not advance");
}
if (client.play("h1", "h2") !== null) {
    throw new Error("played out of turn");
}

socket.receive({
    type: "OpponentMove",
    data: { game_id: "g1", from: 57, to: 42 },
});
if (client.game.state().turn !== "white") {
    throw new Error("opponent move was not applied");
}

socket.receive({
    type: "GameOver",
    data: { game_id: "g1", reason: "resign", result: "win" },
});
if (client.status !== "over" || client.lastResult !== "win") {
    throw new Error("game over was not applied");
}

socket.receive({
    type: "RematchAccepted",
    data: { old_game_id: "g1", new_game_id: "g2", your_color: "black" },
});
if (client.gameId !== "g2" || client.color !== "black") {
    throw new Error("rematch was not applied");
}
if (client.game.state().squares.filter(Boolean).length !== 32) {
    throw new Error("rematch did not reset the board");
}

client.close("bye");
if (!socket.sent.some((message) => message.type === "Disconnect")) {
    throw new Error("disconnect was not sent");
}

console.log("online ok", { playerId: client.playerId, gameId: client.gameId });
