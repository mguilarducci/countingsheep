//! Stamping a validated sheep with trustworthy timestamps at the ingestion
//! edge: `occurred_at` (when it happened) and `received_at` (when we received
//! it). Pure — the caller supplies `now` — so the clock stays at the edge.

use time::OffsetDateTime;

use crate::ingest::sheep::Sheep;

/// A validated sheep carrying two guaranteed, non-optional timestamps.
///
/// `occurred_at` answers "when did the usage happen" — it comes from the
/// client's CloudEvents `time`, or defaults to `received_at` when the client
/// sent none. `received_at` answers "when did we receive it" — it is stamped
/// from our own clock and is never taken from the wire, so a client cannot
/// forge or backdate it. Downstream metering relies on that distinction.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AcceptedSheep {
    pub sheep: Sheep,
    pub occurred_at: OffsetDateTime,
    pub received_at: OffsetDateTime,
}

/// Stamp a validated sheep at the ingestion edge. `now` is supplied by the
/// caller — the handler reads the clock exactly once — keeping this pure and
/// trivially testable with a fixed instant.
pub(crate) fn stamp(sheep: Sheep, now: OffsetDateTime) -> AcceptedSheep {
    let occurred_at = sheep.time.unwrap_or(now);
    AcceptedSheep {
        sheep,
        occurred_at,
        received_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::sheep::Sheep;
    use time::OffsetDateTime;
    use time::macros::datetime;

    fn sheep_with_time(time: Option<OffsetDateTime>) -> Sheep {
        Sheep {
            id: "a-1".into(),
            source: "/svc".into(),
            r#type: "usage.created".into(),
            specversion: "1.0".into(),
            subject: None,
            time,
            data: None,
            datacontenttype: None,
            dataschema: None,
        }
    }

    #[test]
    fn missing_time_defaults_occurred_at_to_now_equal_to_received_at() {
        let now = datetime!(2026-06-26 10:00:00 UTC);
        let stamped = stamp(sheep_with_time(None), now);
        assert_eq!(stamped.occurred_at, now, "defaulted to now");
        assert_eq!(stamped.received_at, now, "received-at is now");
        assert_eq!(
            stamped.occurred_at, stamped.received_at,
            "when the client sends no time, the two stamps are equal"
        );
    }

    #[test]
    fn present_time_is_preserved_as_occurred_at_independent_of_received_at() {
        let occurred = datetime!(2026-06-20 08:30:00 UTC);
        let now = datetime!(2026-06-26 10:00:00 UTC);
        let stamped = stamp(sheep_with_time(Some(occurred)), now);
        assert_eq!(stamped.occurred_at, occurred, "client time kept verbatim");
        assert_eq!(stamped.received_at, now, "received-at is our clock");
        assert_ne!(stamped.occurred_at, stamped.received_at);
    }

    #[test]
    fn future_occurred_at_is_kept_even_when_after_received_at() {
        // Clock-skew policy is out of scope: a future event time is recorded,
        // not clamped to received-at.
        let occurred = datetime!(2099-01-01 00:00:00 UTC);
        let now = datetime!(2026-06-26 10:00:00 UTC);
        let stamped = stamp(sheep_with_time(Some(occurred)), now);
        assert_eq!(stamped.occurred_at, occurred);
        assert!(stamped.occurred_at > stamped.received_at);
    }
}
