import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { connect } from "./lib/eventStream";
import type { components } from "./lib/api/schema";

// Derived from the server's OpenAPI, not hand-written: the CI drift gate on
// schema.ts is only meaningful if the app actually consumes it.
type Health = components["schemas"]["HealthResponse"];

export function App() {
  const queryClient = useQueryClient();
  const [received, setReceived] = useState(0);

  const health = useQuery({
    queryKey: ["health"],
    queryFn: async (): Promise<Health> => {
      const response = await fetch("/api/v1/health");
      if (!response.ok) throw new Error("Could not reach the server.");
      return response.json();
    },
  });

  useEffect(() => {
    const url = `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws`;
    return connect(url, {
      onEvent: () => setReceived((n) => n + 1),
      // The rule from the spec: an event never patches the cache, it only
      // invalidates. The server stays authoritative by construction.
      onResync: () => queryClient.invalidateQueries(),
    });
  }, [queryClient]);

  return (
    <main style={{ fontFamily: "system-ui", maxWidth: "34rem", margin: "4rem auto", lineHeight: 1.6 }}>
      <h1>grubsi</h1>
      <p>
        Server:{" "}
        {health.isPending ? "checking…" : health.isError ? "unreachable" : `ok, v${health.data.version}`}
      </p>
      <p>Events received: {received}</p>
      {/* The server compiles /dev/ping out of release builds, so the button
          that drives it is a dev-only control too. Both go in M4. */}
      {import.meta.env.DEV && (
        <button onClick={() => fetch("/api/v1/dev/ping", { method: "POST" })}>
          Publish an event
        </button>
      )}
    </main>
  );
}
