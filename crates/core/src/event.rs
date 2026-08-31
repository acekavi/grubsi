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

/// The topics one connection is allowed to receive.
///
/// Topic authorization is a connect-time decision (see `Topic`), and this
/// is the value that decision produces. Keeping it a plain set means the
/// filtering rule is a pure predicate that can be tested on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicSet {
    allowed: Vec<Topic>,
}

impl TopicSet {
    pub fn new(allowed: Vec<Topic>) -> Self {
        Self { allowed }
    }

    /// Every restaurant-wide topic and nothing else.
    pub fn staff_only() -> Self {
        Self::new(vec![Topic::Staff])
    }

    /// Whether an event on `topic` may be delivered to this connection.
    pub fn allows(&self, topic: &Topic) -> bool {
        self.allowed.iter().any(|t| t == topic)
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
    fn a_staff_only_set_excludes_customer_topics() {
        let id = uuid::Uuid::nil();
        let set = TopicSet::staff_only();
        assert!(set.allows(&Topic::Staff));
        assert!(!set.allows(&Topic::Table(id)));
        assert!(!set.allows(&Topic::Check(id)));
        assert!(!set.allows(&Topic::Station(id)));
    }

    #[test]
    fn a_set_admits_only_the_exact_topics_it_was_given() {
        let mine = uuid::Uuid::from_u128(1);
        let theirs = uuid::Uuid::from_u128(2);
        let set = TopicSet::new(vec![Topic::Table(mine)]);
        assert!(set.allows(&Topic::Table(mine)));
        assert!(!set.allows(&Topic::Table(theirs)));
        assert!(!set.allows(&Topic::Staff));
    }

    #[test]
    fn event_kinds_serialize_in_screaming_snake_case() {
        let json = serde_json::to_string(&EventKind::Ping).unwrap();
        assert_eq!(json, "\"PING\"");
    }
}
