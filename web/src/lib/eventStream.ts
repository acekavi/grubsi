import type { components } from "./api/schema";

// Both come from the server's OpenAPI. The socket frames used to be matched
// to the server by hand, so nothing asserted that the `"RESYNC"` tag this
// module switches on was the tag the server actually sends. Now a change on
// the Rust side either regenerates this file — which CI's drift gate
// catches — or fails to type-check here.
export type Envelope = components["schemas"]["Envelope"];
export type Frame = components["schemas"]["ClientFrame"];

export type StreamState = { bootId: string | null; lastSeq: number };

export const initialState: StreamState = { bootId: null, lastSeq: 0 };

export type Action = "event" | "resync" | "ignore";

/**
 * Decide what a frame means, given what we have already seen.
 *
 * Two conditions demand a full refetch rather than an incremental update:
 * a sequence gap (an event was dropped) and a changed boot_id (the server
 * restarted, so sequence numbers began again). Both are unrecoverable from
 * the client's cache, and both have the same remedy.
 */
export function reduce(
  state: StreamState,
  frame: Frame,
): { state: StreamState; action: Action } {
  switch (frame.type) {
    case "HELLO":
      return {
        state: { bootId: frame.boot_id, lastSeq: frame.seq },
        action: "ignore",
      };

    case "RESYNC":
      return { state, action: "resync" };

    case "EVENT": {
      const { boot_id: bootId, seq } = frame.envelope;
      const restarted = state.bootId !== null && state.bootId !== bootId;
      const gap = !restarted && seq !== state.lastSeq + 1;

      return {
        state: { bootId, lastSeq: seq },
        action: restarted || gap ? "resync" : "event",
      };
    }
  }
}

type Handlers = {
  onEvent: (envelope: Envelope) => void;
  /** Everything the client holds may be stale. Refetch. */
  onResync: () => void;
};

/** Connect, reconnecting with backoff. Returns a teardown function. */
export function connect(url: string, handlers: Handlers): () => void {
  let state = initialState;
  let socket: WebSocket | null = null;
  let retry = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let closed = false;

  const open = () => {
    if (closed) return;
    socket = new WebSocket(url);

    socket.onopen = () => {
      retry = 0;
    };

    socket.onmessage = (message) => {
      const frame = JSON.parse(message.data as string) as Frame;
      const result = reduce(state, frame);
      state = result.state;

      if (result.action === "event" && frame.type === "EVENT") {
        handlers.onEvent(frame.envelope);
      } else if (result.action === "resync") {
        handlers.onResync();
      }
    };

    socket.onclose = () => {
      if (closed) return;
      // A dropped socket means missed events; refetch once reconnected.
      const delay = Math.min(1000 * 2 ** retry, 15_000);
      retry += 1;
      timer = setTimeout(open, delay);
      handlers.onResync();
    };
  };

  open();

  return () => {
    closed = true;
    if (timer) clearTimeout(timer);
    socket?.close();
  };
}
