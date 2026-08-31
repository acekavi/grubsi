import { describe, expect, it } from "vitest";
import { initialState, reduce, type Frame } from "./eventStream";

const hello = (bootId: string, seq = 0): Frame => ({ type: "HELLO", boot_id: bootId, seq });

const event = (bootId: string, seq: number): Frame => ({
  type: "EVENT",
  envelope: { boot_id: bootId, seq, kind: "PING", topic: "staff", payload: null, at: "" },
});

describe("event stream reducer", () => {
  it("accepts events that arrive in order", () => {
    let s = reduce(initialState, hello("boot-1")).state;
    const first = reduce(s, event("boot-1", 1));
    expect(first.action).toBe("event");
    expect(first.state.lastSeq).toBe(1);

    const second = reduce(first.state, event("boot-1", 2));
    expect(second.action).toBe("event");
    expect(second.state.lastSeq).toBe(2);
  });

  it("asks for a resync when a sequence number is skipped", () => {
    // A dropped event means the client's view is stale in ways it cannot
    // reconstruct. Refetching is the only correct response.
    let s = reduce(initialState, hello("boot-1")).state;
    s = reduce(s, event("boot-1", 1)).state;

    const skipped = reduce(s, event("boot-1", 5));
    expect(skipped.action).toBe("resync");
    expect(skipped.state.lastSeq).toBe(5);
  });

  it("asks for a resync when the server has restarted", () => {
    // A new boot_id means sequence numbers started over; everything the
    // client holds may be stale.
    let s = reduce(initialState, hello("boot-1")).state;
    s = reduce(s, event("boot-1", 4)).state;

    const restarted = reduce(s, event("boot-2", 1));
    expect(restarted.action).toBe("resync");
    expect(restarted.state.bootId).toBe("boot-2");
    expect(restarted.state.lastSeq).toBe(1);
  });

  it("resyncs on a restart even when the sequence number happens to line up", () => {
    // The giveaway is the boot_id, not the sequence. A restarted server
    // begins numbering again, so seq alone cannot distinguish "next event"
    // from "different server, first event" — this is the case that proves
    // boot_id is checked independently.
    let s = reduce(initialState, hello("boot-1")).state;
    s = reduce(s, event("boot-1", 1)).state;

    const restarted = reduce(s, event("boot-2", 2));
    expect(restarted.action).toBe("resync");
    expect(restarted.state.bootId).toBe("boot-2");
    expect(restarted.state.lastSeq).toBe(2);
  });

  it("records HELLO without treating it as an event", () => {
    const result = reduce(initialState, hello("boot-1", 7));
    expect(result.action).toBe("ignore");
    expect(result.state).toEqual({ bootId: "boot-1", lastSeq: 7 });
  });

  it("treats an explicit RESYNC frame as a resync", () => {
    const s = reduce(initialState, hello("boot-1")).state;
    expect(reduce(s, { type: "RESYNC" }).action).toBe("resync");
  });
});
