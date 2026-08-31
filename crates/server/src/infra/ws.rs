use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use grubsi_core::event::{DomainEvent, EventKind, Topic, TopicSet};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;
use utoipa::ToSchema;
use uuid::Uuid;

const DEFAULT_CAPACITY: usize = 512;

/// One published event, as it appears on the wire.
///
/// `boot_id` changes on every server start and `seq` increases by exactly
/// one per event, so a client can detect both a restart and a gap and
/// respond with the same action: refetch.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Envelope {
    pub boot_id: Uuid,
    pub seq: u64,
    /// One of the `EventKind` variants, SCREAMING_SNAKE_CASE (e.g. `PING`).
    // `EventKind` and `Topic` live in `core`, which stays free of the
    // schema machinery; both serialize as strings, so they are described
    // here as strings rather than dragging `utoipa` across the boundary.
    #[schema(value_type = String, example = "PING")]
    pub kind: EventKind,
    /// The topic key: `staff`, or `station:<uuid>` / `table:<uuid>` /
    /// `check:<uuid>`.
    #[schema(value_type = String, example = "staff")]
    pub topic: Topic,
    pub payload: Value,
    pub at: DateTime<Utc>,
}

/// What a socket should send for a given receive result.
#[derive(Debug, Clone)]
pub enum Frame {
    Event(Arc<Envelope>),
    /// The subscriber fell behind. Tell it to refetch rather than treating
    /// this as an error or a disconnect — both defaults are wrong.
    Resync,
}

pub fn frame_for(result: Result<Arc<Envelope>, broadcast::error::RecvError>) -> Option<Frame> {
    match result {
        Ok(env) => Some(Frame::Event(env)),
        Err(broadcast::error::RecvError::Lagged(_)) => Some(Frame::Resync),
        Err(broadcast::error::RecvError::Closed) => None,
    }
}

pub struct EventHub {
    boot_id: Uuid,
    /// The counter and the sender are one unit, not two: see `publish`.
    seq: Mutex<u64>,
    tx: broadcast::Sender<Arc<Envelope>>,
}

impl EventHub {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self {
            boot_id: Uuid::now_v7(),
            seq: Mutex::new(0),
            tx,
        }
    }

    pub fn boot_id(&self) -> Uuid {
        self.boot_id
    }

    /// The last sequence number issued.
    pub fn current_seq(&self) -> u64 {
        *self
            .seq
            .lock()
            .expect("event sequence lock is never poisoned")
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Envelope>> {
        self.tx.subscribe()
    }

    /// Publish one event. Call this only after the transaction that
    /// produced it has committed.
    ///
    /// Taking the number and sending the envelope is one critical section.
    /// If they were two, two concurrent publishers could interleave so that
    /// a subscriber saw seq N+1 before seq N — which the client reads as a
    /// gap and answers with a full refetch. Holding the guard across the
    /// send is safe because `broadcast::Sender::send` is synchronous: there
    /// is no `.await` inside the lock, so no task can be parked holding it.
    pub fn publish(&self, event: DomainEvent) -> Arc<Envelope> {
        let mut seq = self
            .seq
            .lock()
            .expect("event sequence lock is never poisoned");
        *seq += 1;
        let envelope = Arc::new(Envelope {
            boot_id: self.boot_id,
            seq: *seq,
            kind: event.kind,
            topic: event.topic,
            payload: event.payload,
            at: Utc::now(),
        });
        // An error means nobody is listening, which is normal.
        let _ = self.tx.send(Arc::clone(&envelope));
        drop(seq);
        envelope
    }

    pub fn publish_all(&self, events: Vec<DomainEvent>) {
        for event in events {
            self.publish(event);
        }
    }
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;

use crate::state::AppState;

/// Every frame the server sends over `/ws`.
///
/// This is exported into the OpenAPI document so the generated TypeScript
/// client carries the socket contract too. The REST half already has a CI
/// drift gate; without this the `"RESYNC"` tag the client switches on was
/// matched to the server by hand.
#[derive(Serialize, ToSchema)]
#[serde(tag = "type")]
pub enum ClientFrame<'a> {
    #[serde(rename = "HELLO")]
    Hello { boot_id: Uuid, seq: u64 },
    #[serde(rename = "EVENT")]
    Event { envelope: &'a Envelope },
    #[serde(rename = "RESYNC")]
    Resync,
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| pump(socket, state))
}

async fn pump(mut socket: WebSocket, state: AppState) {
    let hub = state.hub;

    // The topics this connection may receive, decided once at connect time.
    // M0 has no authentication, so the honest answer is "staff only": a
    // later milestone derives this from the session, where a customer
    // session resolves to exactly one table topic and one check topic.
    let allowed = TopicSet::staff_only();

    // Subscribe before sending HELLO so no event published between the two
    // can slip past unnoticed.
    let mut rx = hub.subscribe();
    let hello = ClientFrame::Hello {
        boot_id: hub.boot_id(),
        seq: hub.current_seq(),
    };
    if send(&mut socket, &hello).await.is_err() {
        return;
    }

    loop {
        match frame_for(rx.recv().await) {
            Some(Frame::Event(envelope)) => {
                // Topics are a security boundary: an envelope for a topic
                // this connection may not receive is dropped, not sent.
                if !allowed.allows(&envelope.topic) {
                    continue;
                }
                if send(
                    &mut socket,
                    &ClientFrame::Event {
                        envelope: &envelope,
                    },
                )
                .await
                .is_err()
                {
                    return;
                }
            }
            Some(Frame::Resync) => {
                tracing::warn!("websocket subscriber lagged; asking client to resync");
                if send(&mut socket, &ClientFrame::Resync).await.is_err() {
                    return;
                }
            }
            None => return,
        }
    }
}

async fn send(socket: &mut WebSocket, frame: &ClientFrame<'_>) -> Result<(), axum::Error> {
    let text = serde_json::to_string(frame).expect("frames are serializable");
    socket.send(Message::Text(text.into())).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use grubsi_core::event::DomainEvent;

    #[tokio::test]
    async fn every_subscriber_receives_the_same_envelope() {
        let hub = EventHub::new();
        let mut a = hub.subscribe();
        let mut b = hub.subscribe();

        hub.publish(DomainEvent::ping());

        let ea = a.recv().await.unwrap();
        let eb = b.recv().await.unwrap();
        assert_eq!(ea.seq, eb.seq);
        assert_eq!(ea.seq, 1);
    }

    #[tokio::test]
    async fn sequence_numbers_increase_monotonically() {
        let hub = EventHub::new();
        let mut rx = hub.subscribe();

        hub.publish(DomainEvent::ping());
        hub.publish(DomainEvent::ping());
        hub.publish(DomainEvent::ping());

        let seqs = vec![
            rx.recv().await.unwrap().seq,
            rx.recv().await.unwrap().seq,
            rx.recv().await.unwrap().seq,
        ];
        assert_eq!(seqs, vec![1, 2, 3]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_publishers_cannot_deliver_out_of_order() {
        // Two devices pressing the same button at once must not make a
        // subscriber see seq N+1 before seq N — the client reads that as a
        // gap and forces a full refetch.
        const PUBLISHERS: u64 = 8;
        const EACH: u64 = 50;
        let hub = Arc::new(EventHub::with_capacity(1024));
        let mut rx = hub.subscribe();

        let mut tasks = Vec::new();
        for _ in 0..PUBLISHERS {
            let hub = Arc::clone(&hub);
            tasks.push(tokio::spawn(async move {
                for _ in 0..EACH {
                    hub.publish(DomainEvent::ping());
                }
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }

        for expected in 1..=(PUBLISHERS * EACH) {
            assert_eq!(rx.recv().await.unwrap().seq, expected);
        }
    }

    #[tokio::test]
    async fn a_lagging_subscriber_is_told_to_resync_rather_than_dropped() {
        // A slow KDS tablet must recover by refetching, not by silently
        // missing orders and not by tearing down its socket.
        let hub = EventHub::with_capacity(2);
        let mut rx = hub.subscribe();

        for _ in 0..10 {
            hub.publish(DomainEvent::ping());
        }

        match rx.recv().await {
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                assert!(n > 0);
                let frame = frame_for(Err(tokio::sync::broadcast::error::RecvError::Lagged(n)));
                assert!(matches!(frame, Some(Frame::Resync)), "got {frame:?}");
            }
            other => panic!("expected Lagged, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_connection_receives_only_envelopes_on_its_own_topics() {
        // The filtering predicate is the security boundary; test it
        // directly against real envelopes rather than through a socket.
        let hub = EventHub::new();
        let allowed = TopicSet::staff_only();

        let staff = hub.publish(DomainEvent::ping());
        let table = hub.publish(DomainEvent::new(
            EventKind::Ping,
            Topic::Table(Uuid::now_v7()),
            serde_json::Value::Null,
        ));

        assert!(allowed.allows(&staff.topic), "staff event must be sent");
        assert!(
            !allowed.allows(&table.topic),
            "a table event must not reach a staff-only connection"
        );
    }

    #[test]
    fn a_closed_channel_ends_the_stream() {
        assert!(frame_for(Err(tokio::sync::broadcast::error::RecvError::Closed)).is_none());
    }
}
