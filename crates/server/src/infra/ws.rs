use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use grubsi_core::event::{DomainEvent, EventKind, Topic};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;
use uuid::Uuid;

const DEFAULT_CAPACITY: usize = 512;

/// One published event, as it appears on the wire.
///
/// `boot_id` changes on every server start and `seq` increases by exactly
/// one per event, so a client can detect both a restart and a gap and
/// respond with the same action: refetch.
#[derive(Debug, Clone, Serialize)]
pub struct Envelope {
    pub boot_id: Uuid,
    pub seq: u64,
    pub kind: EventKind,
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
    seq: AtomicU64,
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
            seq: AtomicU64::new(0),
            tx,
        }
    }

    pub fn boot_id(&self) -> Uuid {
        self.boot_id
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Envelope>> {
        self.tx.subscribe()
    }

    /// Publish one event. Call this only after the transaction that
    /// produced it has committed.
    pub fn publish(&self, event: DomainEvent) -> Arc<Envelope> {
        let envelope = Arc::new(Envelope {
            boot_id: self.boot_id,
            seq: self.seq.fetch_add(1, Ordering::SeqCst) + 1,
            kind: event.kind,
            topic: event.topic,
            payload: event.payload,
            at: Utc::now(),
        });
        // An error means nobody is listening, which is normal.
        let _ = self.tx.send(Arc::clone(&envelope));
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

    #[test]
    fn a_closed_channel_ends_the_stream() {
        assert!(frame_for(Err(tokio::sync::broadcast::error::RecvError::Closed)).is_none());
    }
}
