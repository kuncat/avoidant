//! Authoritative gameplay state. Rendering timers never mutate this state.
use crate::{CellMetadataEntry, ScoreState};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const PROTOCOL: u8 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Request {
    pub id: u64,
    pub index: usize,
    pub position: [f64; 3],
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Snapshot {
    pub revision: u64,
    #[serde(with = "bitmap")]
    pub explored: Vec<bool>,
    pub score: ScoreState,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum Message {
    Request {
        version: u8,
        request: Request,
    },
    Sync {
        version: u8,
    },
    State {
        version: u8,
        snapshot: Snapshot,
        receipt: Option<(String, u64)>,
        movement: Option<Request>,
    },
}

pub(crate) struct Session {
    pub authority: Option<String>,
    pub local_id: Option<String>,
    pub ready: bool,
    pub stopped: bool,
    pub metadata: Vec<CellMetadataEntry>,
    neighbors: Vec<Vec<u32>>,
    pub score: ScoreState,
    pub revision: u64,
    next_request: u64,
    pub pending: BTreeMap<u64, Request>,
    processed: BTreeSet<(String, u64)>,
}

impl Session {
    pub fn new(authority: Option<String>) -> Self {
        Self {
            ready: authority.is_none(),
            authority,
            local_id: None,
            stopped: false,
            metadata: vec![],
            neighbors: vec![],
            score: ScoreState::default(),
            revision: 0,
            next_request: 0,
            pending: BTreeMap::new(),
            processed: BTreeSet::new(),
        }
    }

    pub fn is_host(&self) -> bool {
        self.authority.is_none() || self.authority == self.local_id
    }

    pub fn set_board(
        &mut self,
        neighbors: Vec<Vec<u32>>,
        metadata: Vec<CellMetadataEntry>,
        void_total: u32,
    ) {
        self.score.reset_for_map(metadata.len() as u32, void_total);
        self.neighbors = neighbors;
        self.metadata = metadata;
        self.revision = 0;
        self.pending.clear();
        self.processed.clear();
    }

    pub fn request(&mut self, index: usize, position: [f64; 3]) -> Option<Request> {
        if !self.ready
            || self.score.completed
            || index >= self.metadata.len()
            || self.metadata[index].is_explored
            || self.pending.values().any(|r| r.index == index)
            || !position.iter().all(|v| v.is_finite())
        {
            return None;
        }
        self.next_request += 1;
        let request = Request {
            id: self.next_request,
            index,
            position,
        };
        if !self.is_host() {
            self.pending.insert(request.id, request.clone());
        }
        Some(request)
    }

    pub fn apply(&mut self, actor: &str, request: &Request) -> bool {
        if !self.is_host()
            || request.index >= self.metadata.len()
            || !request.position.iter().all(|v| v.is_finite())
            || !self.processed.insert((actor.to_owned(), request.id))
        {
            return false;
        }
        if self.score.completed || self.metadata[request.index].is_explored {
            return false;
        }
        let revealed = crate::mutation::reveal_set(
            self.metadata.len(),
            request.index,
            |index| Ok(self.metadata[index].clone()),
            |index| Ok(self.neighbors[index].clone()),
        )
        .expect("in-memory board is valid");
        for index in revealed {
            self.metadata[index].is_explored = true;
            self.score.explore(self.metadata[index].is_void);
        }
        self.revision += 1;
        true
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            revision: self.revision,
            explored: self.metadata.iter().map(|cell| cell.is_explored).collect(),
            score: self.score.clone(),
        }
    }

    pub fn accept(
        &mut self,
        from: &str,
        snapshot: &Snapshot,
        receipt: &Option<(String, u64)>,
    ) -> bool {
        if self.is_host()
            || self.authority.as_deref() != Some(from)
            || snapshot.explored.len() != self.metadata.len()
            || snapshot.score.total_cells as usize != self.metadata.len()
            || snapshot.score.void_total != self.score.void_total
        {
            return false;
        }
        if let Some((actor, id)) = receipt {
            if self.local_id.as_ref() == Some(actor) {
                self.pending.remove(id);
            }
        }
        if self.ready && snapshot.revision <= self.revision {
            return false;
        }
        for (cell, explored) in self.metadata.iter_mut().zip(&snapshot.explored) {
            cell.is_explored = *explored;
        }
        self.score = snapshot.score.clone();
        self.revision = snapshot.revision;
        self.ready = true;
        true
    }
}

// Keep maximum-size (5,000-cell) snapshots below gossip's 4 KiB message limit.
mod bitmap {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(bits: &[bool], serializer: S) -> Result<S::Ok, S::Error> {
        let mut packed = format!("{}:", bits.len());
        for chunk in bits.chunks(4) {
            let value = chunk
                .iter()
                .enumerate()
                .fold(0u32, |v, (i, &bit)| v | ((bit as u32) << i));
            packed.push(char::from_digit(value, 16).unwrap());
        }
        serializer.serialize_str(&packed)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<bool>, D::Error> {
        let text = String::deserialize(deserializer)?;
        let (len, data) = text
            .split_once(':')
            .ok_or_else(|| serde::de::Error::custom("invalid bitmap"))?;
        let len: usize = len.parse().map_err(serde::de::Error::custom)?;
        if len > 100_000 || data.len() != len.div_ceil(4) {
            return Err(serde::de::Error::custom("invalid bitmap length"));
        }
        let mut bits = Vec::with_capacity(data.len() * 4);
        for c in data.chars() {
            let n = c
                .to_digit(16)
                .ok_or_else(|| serde::de::Error::custom("invalid bitmap digit"))?;
            bits.extend((0..4).map(|i| (n & (1 << i)) != 0));
        }
        bits.truncate(len);
        Ok(bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board(authority: Option<&str>, local: &str) -> Session {
        let mut s = Session::new(authority.map(str::to_owned));
        s.local_id = Some(local.into());
        // Two numbered safe cells, a void, then another safe cell.
        s.set_board(
            vec![vec![2], vec![2], vec![0, 1, 3], vec![2]],
            vec![
                CellMetadataEntry::new(false, false, 1),
                CellMetadataEntry::new(false, false, 1),
                CellMetadataEntry::new(false, true, 0),
                CellMetadataEntry::new(false, false, 1),
            ],
            1,
        );
        s
    }
    fn request(id: u64, index: usize) -> Request {
        Request {
            id,
            index,
            position: [0.0; 3],
        }
    }
    fn same(host: &Session, peer: &Session) {
        assert_eq!(host.snapshot().explored, peer.snapshot().explored);
        assert_eq!(
            serde_json::to_value(&host.score).unwrap(),
            serde_json::to_value(&peer.score).unwrap()
        );
        assert_eq!(host.revision, peer.revision);
    }
    #[test]
    fn late_join_receives_entire_board_and_score_without_replaying_moves() {
        let mut host = board(None, "host");
        host.apply("host", &request(1, 0));
        host.apply("host", &request(2, 2));
        host.apply("host", &request(3, 1));
        let mut peer = board(Some("host"), "peer");
        assert!(!peer.ready);
        assert!(peer.request(3, [0.0; 3]).is_none());
        assert!(peer.accept("host", &host.snapshot(), &None));
        same(&host, &peer);
        assert!(peer.ready);
    }
    #[test]
    fn concurrent_duplicate_requests_and_out_of_order_snapshots_converge() {
        let mut host = board(None, "host");
        let mut a = board(Some("host"), "a");
        let mut b = board(Some("host"), "b");
        a.accept("host", &host.snapshot(), &None);
        b.accept("host", &host.snapshot(), &None);
        let ra = a.request(0, [0.0; 3]).unwrap();
        let rb = b.request(0, [0.0; 3]).unwrap();
        assert!(host.apply("b", &rb));
        let older = host.snapshot();
        assert!(!host.apply("a", &ra));
        assert!(!host.apply("b", &rb));
        assert_eq!(host.score.safe_explored, 1);
        host.apply("a", &request(2, 2)); // streak resets once
        host.apply("b", &request(2, 1));
        let latest = host.snapshot();
        a.accept("host", &latest, &Some(("a".into(), ra.id)));
        b.accept("host", &latest, &Some(("b".into(), rb.id)));
        assert!(!a.accept("host", &older, &None));
        assert!(!b.accept("host", &older, &None));
        assert!(a.pending.is_empty() && b.pending.is_empty());
        same(&host, &a);
        same(&host, &b);
    }
    #[test]
    fn lost_move_or_receipt_can_be_retried_and_missing_snapshot_repaired() {
        let mut host = board(None, "host");
        let mut peer = board(Some("host"), "peer");
        peer.accept("host", &host.snapshot(), &None);
        let r = peer.request(1, [0.0; 3]).unwrap();
        assert!(peer.pending.contains_key(&r.id));
        assert!(host.apply("peer", peer.pending.get(&r.id).unwrap()));
        // Snapshot/receipt lost; retry is acknowledged without scoring again.
        assert!(!host.apply("peer", peer.pending.get(&r.id).unwrap()));
        peer.accept("host", &host.snapshot(), &None);
        // A receipt in an equal-revision snapshot still clears the pending request.
        assert!(!peer.accept("host", &host.snapshot(), &Some(("peer".into(), r.id))));
        assert!(peer.pending.is_empty());
        same(&host, &peer);
    }
    #[test]
    fn reject_non_host_and_malformed_snapshots() {
        let host = board(None, "host");
        let mut peer = board(Some("host"), "peer");
        assert!(!peer.accept("impostor", &host.snapshot(), &None));
        let mut invalid = host.snapshot();
        invalid.explored.pop();
        assert!(!peer.accept("host", &invalid, &None));
        assert!(!peer.ready);
    }
    #[test]
    fn snapshot_roundtrip_fits_transport_and_preserves_all_cells() {
        for count in [1, 80, 160, 320, 5000] {
            let bits: Vec<bool> = (0..count).map(|i| i % 3 == 0).collect();
            let message = Message::State {
                version: PROTOCOL,
                snapshot: Snapshot {
                    revision: 123,
                    explored: bits.clone(),
                    score: ScoreState::default(),
                },
                receipt: Some(("a".repeat(64), 5000)),
                movement: Some(request(5000, count - 1)),
            };
            let wire = serde_json::to_string(&message).unwrap();
            assert!(wire.len() < 3000); // Allow room for signed transport envelope.
            let Message::State { snapshot, .. } = serde_json::from_str(&wire).unwrap() else {
                panic!()
            };
            assert_eq!(snapshot.explored, bits);
        }
    }
    #[test]
    fn completion_and_overlapping_flood_reveals_are_scored_once() {
        let mut host = board(None, "host");
        host.metadata.iter_mut().for_each(|cell| {
            cell.is_void = false;
            cell.void_neighbor_count = 0;
        });
        host.score.reset_for_map(4, 0);
        assert!(host.apply("a", &request(1, 0)));
        assert!(!host.apply("b", &request(1, 1)));
        assert_eq!(host.score.safe_explored, 4);
        assert!(host.score.completed);
        assert_eq!(host.score.score, 1.0 + 1.1 + 1.2 + 1.3 + 1.0);
        let mut peer = board(Some("host"), "peer");
        peer.score.reset_for_map(4, 0);
        peer.accept("host", &host.snapshot(), &None);
        same(&host, &peer);
    }
}
