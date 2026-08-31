use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

/// Who an event is for.
///
/// Topics are a security boundary, not a convenience: a customer device
/// must never receive restaurant-wide state. The server derives the set a
/// socket may join from its session, at connect time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Topic {
    Staff,
    Station(Uuid),
    Table(Uuid),
    Check(Uuid),
}

impl Topic {
    pub fn as_key(&self) -> String {
        match self {
            Topic::Staff => "staff".to_owned(),
            Topic::Station(id) => format!("station:{id}"),
            Topic::Table(id) => format!("table:{id}"),
            Topic::Check(id) => format!("check:{id}"),
        }
    }
}

impl Serialize for Topic {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.as_key())
    }
}

/// M0 defines only `Ping`. Real kinds arrive with the features that emit
/// them; see the spec, section 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventKind {
    Ping,
}

#[derive(Debug, Clone, Serialize)]
pub struct DomainEvent {
    pub kind: EventKind,
    pub topic: Topic,
    pub payload: Value,
}

impl DomainEvent {
    pub fn new(kind: EventKind, topic: Topic, payload: Value) -> Self {
        Self {
            kind,
            topic,
            payload,
        }
    }

    /// The M0 walking-skeleton event.
    pub fn ping() -> Self {
        Self::new(EventKind::Ping, Topic::Staff, Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topics_render_as_stable_keys() {
        let id = uuid::Uuid::nil();
        assert_eq!(Topic::Staff.as_key(), "staff");
        assert_eq!(Topic::Station(id).as_key(), format!("station:{id}"));
        assert_eq!(Topic::Table(id).as_key(), format!("table:{id}"));
        assert_eq!(Topic::Check(id).as_key(), format!("check:{id}"));
    }

    #[test]
    fn event_kinds_serialize_in_screaming_snake_case() {
        let json = serde_json::to_string(&EventKind::Ping).unwrap();
        assert_eq!(json, "\"PING\"");
    }
}
