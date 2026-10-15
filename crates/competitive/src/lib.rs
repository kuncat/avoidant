//! Trusted competitive rules. Private state must never be sent to a player.
use rand::{SeedableRng, seq::SliceRandom};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use wasm_bindgen::prelude::*;

pub const VERSION: u32 = 1;
fn default_match_minutes() -> u64 {
    20
}
fn default_turn_seconds() -> Option<u64> {
    Some(30)
}
const COOLDOWN_MS: u64 = 400;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Collaborative,
    Territory,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    pub mode: Mode,
    pub bordering: bool,
    /// Additional reveals, excluding the selected cell. None means unlimited.
    pub flood_limit: Option<usize>,
    pub turn_based: bool,
    #[serde(default = "default_match_minutes")]
    pub match_minutes: u64,
    #[serde(default = "default_turn_seconds")]
    pub turn_seconds: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub score: i32,
    pub claims: u32,
    pub voids: u32,
    pub sequence: u32,
    #[serde(default)]
    pub cumulative_ms: u64,
    pub last_action: Option<u64>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    version: u32,
    options: Options,
    neighbors: Vec<Vec<usize>>,
    voids: Vec<bool>,
    revealed: Vec<bool>,
    owners: Vec<Option<usize>>,
    players: Vec<Player>,
    revision: u32,
    started: bool,
    finished: bool,
    turn: usize,
    turn_deadline: u64,
    deadline: u64,
    passes: usize,
    #[serde(default)]
    started_at: Option<u64>,
    #[serde(default)]
    finished_at: Option<u64>,
    #[serde(default)]
    accounted_at: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicCell {
    pub revealed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_void: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    pub owner: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub version: u32,
    pub options: Options,
    pub cells: Vec<PublicCell>,
    pub players: Vec<Player>,
    pub revision: u32,
    pub started: bool,
    pub finished: bool,
    pub turn: Option<usize>,
    pub turn_deadline: u64,
    pub deadline: u64,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Action {
    Select { cell: usize },
    Pass,
}

impl Match {
    pub fn new(
        neighbors: Vec<Vec<usize>>,
        fraction: f64,
        seed: [u8; 32],
        mut options: Options,
        creator: String,
    ) -> Result<Self, String> {
        let n = neighbors.len();
        if !(16..=4096).contains(&n)
            || !fraction.is_finite()
            || !(0.05..=0.4).contains(&fraction)
            || creator.is_empty()
        {
            return Err("Invalid board or creator".into());
        }
        for (i, edges) in neighbors.iter().enumerate() {
            if edges.is_empty()
                || edges.len() > 32
                || edges
                    .iter()
                    .any(|&j| j >= n || j == i || !neighbors[j].contains(&i))
            {
                return Err("Invalid adjacency".into());
            }
        }
        if !(1..=1440).contains(&options.match_minutes)
            || options
                .turn_seconds
                .is_some_and(|s| !(1..=86400).contains(&s))
        {
            return Err("Invalid match or turn duration".into());
        }
        if options.flood_limit.is_some_and(|v| v > 4096) {
            return Err("Flood limit exceeds board maximum".into());
        }
        if options.mode == Mode::Collaborative {
            options.turn_based = false;
            options.bordering = false;
        }
        let mut indices: Vec<usize> = (0..n).collect();
        indices.shuffle(&mut ChaCha20Rng::from_seed(seed));
        let mut voids = vec![false; n];
        for &i in indices.iter().take((fraction * n as f64).floor() as usize) {
            voids[i] = true;
        }
        let mut game = Self {
            version: VERSION,
            options,
            neighbors,
            voids,
            revealed: vec![false; n],
            owners: vec![None; n],
            players: vec![],
            revision: 0,
            started: false,
            finished: false,
            turn: 0,
            turn_deadline: 0,
            deadline: 0,
            passes: 0,
            started_at: None,
            finished_at: None,
            accounted_at: None,
        };
        game.join(creator)?;
        Ok(game)
    }

    pub fn join(&mut self, id: String) -> Result<(), String> {
        if self.players.iter().any(|p| p.id == id) {
            return Ok(());
        }
        let max = 8;
        if (self.started && self.options.mode != Mode::Collaborative)
            || self.finished
            || self.players.len() >= max
            || id.is_empty()
        {
            return Err("Lobby closed".into());
        }
        self.players.push(Player {
            id,
            name: String::new(),
            score: 0,
            claims: 0,
            voids: 0,
            sequence: 0,
            cumulative_ms: 0,
            last_action: None,
        });
        self.revision += 1;
        Ok(())
    }

    fn join_named(&mut self, id: String, name: Option<String>) -> Result<(), String> {
        let name = name.map(|n| n.trim().to_string());
        if let Some(n) = &name {
            if n.is_empty() || n.chars().count() > 32 || n.chars().any(|c| c.is_control()) {
                return Err("Invalid player name".into());
            }
        }
        let existing = self.players.iter().any(|p| p.id == id);
        self.join(id.clone())?;
        // Names remain stable during play, including when a socket reconnects.
        if (!existing || !self.started)
            && let Some(name) = name
        {
            let player = self.players.iter_mut().find(|p| p.id == id).unwrap();
            if player.name != name {
                player.name = name;
                self.revision += 1;
            }
        }
        Ok(())
    }

    pub fn start(&mut self, actor: &str, now: u64) -> Result<(), String> {
        if self.players[0].id != actor || self.started || self.players.len() < 2 {
            return Err("Cannot start match".into());
        }
        self.started = true;
        self.started_at = Some(now);
        self.accounted_at = Some(now);
        self.finished_at = None;
        self.deadline = now + self.options.match_minutes * 60_000;
        self.turn_deadline = self.options.turn_seconds.map_or(0, |s| now + s * 1000);
        // Spread starts by graph distance; no claim of equal terrain difficulty.
        let mut starts: Vec<usize> = vec![];
        let count = if !self.options.bordering {
            0
        } else if self.options.mode != Mode::Territory {
            1
        } else {
            self.players.len()
        };
        for p in 0..count {
            let mut distances = vec![usize::MAX; self.neighbors.len()];
            let mut queue = VecDeque::new();
            for &s in &starts {
                distances[s] = 0;
                queue.push_back(s);
            }
            while let Some(i) = queue.pop_front() {
                for &j in &self.neighbors[i] {
                    if distances[j] == usize::MAX {
                        distances[j] = distances[i] + 1;
                        queue.push_back(j);
                    }
                }
            }
            let s = (0..self.voids.len())
                .filter(|&i| !self.voids[i] && !starts.contains(&i))
                .max_by_key(|&i| (distances[i], std::cmp::Reverse(i)))
                .ok_or("Insufficient starts")?;
            starts.push(s);
            self.revealed[s] = true;
            if self.options.mode == Mode::Territory {
                self.owners[s] = Some(p);
                self.players[p].claims = 1;
                self.players[p].score = 1;
            }
        }
        self.revision += 1;
        Ok(())
    }

    fn account_time(&mut self, until: u64) {
        let previous = self.accounted_at.unwrap_or(until);
        let elapsed = until.saturating_sub(previous);
        if self.options.turn_based {
            self.players[self.turn].cumulative_ms += elapsed;
        } else {
            for player in &mut self.players {
                player.cumulative_ms += elapsed;
            }
        }
        self.accounted_at = Some(previous.max(until));
    }

    pub fn tick(&mut self, now: u64) {
        if !self.started || self.finished {
            return;
        }
        let until = now.min(self.deadline);
        if self.options.turn_based
            && let Some(seconds) = self.options.turn_seconds
        {
            // At most one full round: consecutive timeouts finish the match.
            while self.turn_deadline <= until {
                self.account_time(self.turn_deadline);
                self.passes += 1;
                let ended = self.turn_deadline;
                self.turn = (self.turn + 1) % self.players.len();
                self.turn_deadline += seconds * 1000;
                self.revision += 1;
                if self.passes >= self.players.len() {
                    self.finished = true;
                    self.finished_at = Some(ended);
                    return;
                }
            }
        }
        self.account_time(until);
        if now >= self.deadline {
            self.finished = true;
            self.finished_at = Some(self.deadline);
            self.revision += 1;
        }
    }

    pub fn act(
        &mut self,
        actor: &str,
        sequence: u32,
        action: Action,
        now: u64,
    ) -> Result<(), String> {
        self.tick(now);
        let p = self
            .players
            .iter()
            .position(|p| p.id == actor)
            .ok_or("Not a player")?;
        if sequence <= self.players[p].sequence {
            return Ok(());
        }
        if sequence != self.players[p].sequence + 1 {
            return Err("Out of sequence".into());
        }
        if !self.started || self.finished {
            return Err("Match is not active".into());
        }
        if self.options.turn_based && self.turn != p {
            return Err("Not your turn".into());
        }
        if self.players[p]
            .last_action
            .is_some_and(|t| now < t + COOLDOWN_MS)
        {
            return Err("Action cooldown".into());
        }
        match action {
            Action::Pass => {
                if !self.options.turn_based {
                    return Err("Passing requires turns".into());
                }
                self.passes += 1;
            }
            Action::Select { cell } => {
                if cell >= self.voids.len()
                    || self.owners[cell].is_some()
                    || (self.revealed[cell]
                        && (self.voids[cell] || self.options.mode != Mode::Territory))
                {
                    return Err("Cell unavailable".into());
                }
                if self.options.bordering
                    && !self.neighbors[cell]
                        .iter()
                        .any(|&j| self.owners[j] == Some(p))
                {
                    return Err("Cell must border your territory".into());
                }
                self.passes = 0;
                let before = self.revealed.iter().filter(|&&v| v).count();
                // Only an unopened board may change: already-visible clues stay valid.
                // Stable index order makes relocation reproducible from saved state + command.
                if self.voids[cell] && !self.revealed.iter().any(|&v| v) {
                    let replacement = (0..self.voids.len()).find(|&i| !self.voids[i]).unwrap();
                    self.voids[cell] = false;
                    self.voids[replacement] = true;
                }
                self.reveal(cell);
                if self.voids[cell] {
                    if self.options.mode == Mode::Collaborative {
                        self.players[p].score -= 5;
                    }
                    self.players[p].voids += 1;
                } else {
                    self.players[p].score += if self.options.mode == Mode::Collaborative {
                        (self.revealed.iter().filter(|&&v| v).count() - before) as i32
                    } else {
                        1
                    };
                    if self.options.mode == Mode::Territory {
                        self.owners[cell] = Some(p);
                        self.players[p].claims += 1;
                    }
                }
            }
        }
        if self.options.mode == Mode::Territory {
            for player in &mut self.players {
                player.score = player.claims as i32;
            }
        }
        self.players[p].sequence = sequence;
        self.players[p].last_action = Some(now);
        self.turn = (self.turn + 1) % self.players.len();
        self.turn_deadline = self.options.turn_seconds.map_or(0, |s| now + s * 1000);
        self.finished = self.passes >= self.players.len()
            || (0..self.voids.len()).all(|i| {
                self.voids[i]
                    || if self.options.mode != Mode::Territory {
                        self.revealed[i]
                    } else {
                        self.owners[i].is_some()
                    }
            });
        if self.finished {
            self.finished_at = Some(now.min(self.deadline));
        }
        self.revision += 1;
        Ok(())
    }

    fn count(&self, cell: usize) -> usize {
        self.neighbors[cell]
            .iter()
            .filter(|&&n| self.voids[n])
            .count()
    }

    fn reveal(&mut self, cell: usize) {
        self.revealed[cell] = true;
        if self.voids[cell] || self.count(cell) > 0 {
            return;
        }
        let limit = self.options.flood_limit.unwrap_or(usize::MAX);
        let mut added = 0;
        let mut visited = vec![false; self.voids.len()];
        visited[cell] = true;
        let mut queue = VecDeque::from([cell]);
        while let Some(i) = queue.pop_front() {
            if added >= limit {
                break;
            }
            let mut edges = self.neighbors[i].clone();
            edges.sort_unstable();
            for j in edges {
                if visited[j] {
                    continue;
                }
                visited[j] = true;
                if !self.revealed[j] {
                    if added >= limit {
                        return;
                    }
                    self.revealed[j] = true;
                    added += 1;
                }
                if self.count(j) == 0 {
                    queue.push_back(j);
                }
            }
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            version: self.version,
            options: self.options.clone(),
            cells: (0..self.voids.len())
                .map(|i| PublicCell {
                    revealed: self.revealed[i],
                    is_void: self.revealed[i].then_some(self.voids[i]),
                    count: (self.revealed[i] && !self.voids[i]).then(|| self.count(i)),
                    owner: self.owners[i],
                })
                .collect(),
            players: self.players.clone(),
            revision: self.revision,
            started: self.started,
            finished: self.finished,
            turn: self.options.turn_based.then_some(self.turn),
            turn_deadline: self.turn_deadline,
            deadline: self.deadline,
            started_at: self.started_at,
            finished_at: self.finished_at,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Command {
    Create {
        neighbors: Vec<Vec<usize>>,
        fraction: f64,
        seed: String,
        options: Options,
        actor: String,
    },
    Join {
        actor: String,
        name: Option<String>,
        #[serde(default)]
        now: u64,
    },
    Start {
        actor: String,
        now: u64,
    },
    Move {
        actor: String,
        sequence: u32,
        action: Action,
        now: u64,
    },
    Tick {
        now: u64,
    },
}

/// This return value contains PRIVATE state. Only `snapshot` may leave the server.
#[wasm_bindgen]
pub fn transition(state: &str, command: &str) -> Result<String, JsError> {
    run(state, command).map_err(|e| JsError::new(&e))
}

fn parse_seed(seed: &str) -> Result<[u8; 32], String> {
    if seed.len() != 64 || !seed.is_ascii() {
        return Err("Invalid seed".into());
    }
    let mut bytes = [0u8; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&seed[i * 2..i * 2 + 2], 16).map_err(|_| "Invalid seed")?;
    }
    Ok(bytes)
}

fn run(state: &str, command: &str) -> Result<String, String> {
    let command: Command = serde_json::from_str(command).map_err(|_| "Invalid command")?;
    let mut error = None;
    let game = match command {
        Command::Create {
            neighbors,
            fraction,
            seed,
            options,
            actor,
        } => Match::new(neighbors, fraction, parse_seed(&seed)?, options, actor)?,
        c => {
            let mut g: Match = serde_json::from_str(state).map_err(|_| "Invalid saved match")?;
            if g.version != VERSION {
                return Err("Incompatible saved match".into());
            }
            let result = match c {
                Command::Join { actor, name, now } => {
                    g.tick(now);
                    g.join_named(actor, name)
                }
                Command::Start { actor, now } => g.start(&actor, now),
                Command::Move {
                    actor,
                    sequence,
                    action,
                    now,
                } => {
                    g.tick(now);
                    g.act(&actor, sequence, action, now)
                }
                Command::Tick { now } => {
                    g.tick(now);
                    Ok(())
                }
                _ => unreachable!(),
            };
            error = result.err();
            g
        }
    };
    serde_json::to_string(&serde_json::json!({ "state": serde_json::to_string(&game).unwrap(), "snapshot": game.snapshot(), "error": error })).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_void_selection_is_safe_and_replays_exactly() {
        for mode in [Mode::Territory, Mode::Collaborative] {
            let mut g = game(Some(0), false);
            g.options.mode = mode;
            let initial = serde_json::to_string(&g).unwrap();
            let cell = g.voids.iter().position(|&v| v).unwrap();
            let count = g.voids.iter().filter(|&&v| v).count();
            let command = serde_json::json!({"kind":"move", "actor":"b", "sequence":1,
                "action":{"kind":"select", "cell":cell}, "now":2000})
            .to_string();
            let first = run(&initial, &command).unwrap();
            assert_eq!(first, run(&initial, &command).unwrap());
            let result: serde_json::Value = serde_json::from_str(&first).unwrap();
            let mut restored: Match =
                serde_json::from_str(result["state"].as_str().unwrap()).unwrap();
            assert!(!restored.voids[cell]);
            assert!(restored.revealed[cell]);
            assert_eq!(restored.voids.iter().filter(|&&v| v).count(), count);
            // Later selections do not move voids or alter previous clues.
            let board = restored.voids.clone();
            let next = board.iter().position(|&v| v).unwrap();
            restored
                .act("b", 2, Action::Select { cell: next }, 3000)
                .unwrap();
            assert_eq!(restored.voids, board);
            assert_eq!(restored.players[1].voids, 1);
            assert_eq!(
                restored.players[1].score,
                if mode == Mode::Territory { 1 } else { -4 }
            );
            assert_eq!(
                restored.players[1].claims,
                if mode == Mode::Territory { 1 } else { 0 }
            );
        }
    }

    #[test]
    fn player_names_are_validated_persisted_and_stable_during_play() {
        let mut g = game(Some(0), false);
        g.started = false;
        g.join_named("a".into(), Some("  আশা  ".into())).unwrap();
        assert_eq!(g.snapshot().players[0].name, "আশা");
        for invalid in [
            "".to_string(),
            " ".to_string(),
            "a".repeat(33),
            "a\nb".into(),
        ] {
            assert!(g.join_named("c".into(), Some(invalid)).is_err());
        }
        assert_eq!(g.players.len(), 2);
        let state = serde_json::to_string(&g).unwrap();
        let mut restored: Match = serde_json::from_str(&state).unwrap();
        restored.started = true;
        restored
            .join_named("a".into(), Some("Changed".into()))
            .unwrap();
        assert_eq!(restored.snapshot().players[0].name, "আশা");
        restored.options.mode = Mode::Collaborative;
        restored
            .join_named("c".into(), Some("New player".into()))
            .unwrap();
        assert_eq!(restored.players[2].name, "New player");
    }

    #[test]
    fn configured_deadlines_and_disabled_turn_timer() {
        let mut g = game(Some(0), true);
        g.started = false;
        g.options.match_minutes = 1440;
        g.options.turn_seconds = Some(90);
        g.start("a", 1000).unwrap();
        assert_eq!(g.deadline, 1000 + 86_400_000);
        assert_eq!(g.turn_deadline, 91_000);
        g.tick(90_999);
        assert_eq!(g.turn, 0);
        g.tick(91_000);
        assert_eq!(g.turn, 1);
        g.started = false;
        g.options.turn_seconds = None;
        g.start("a", 1000).unwrap();
        let turn = g.turn;
        g.tick(1_000_000);
        assert_eq!(g.turn, turn);
        assert_eq!(g.turn_deadline, 0);
        assert!(!g.finished);
        g.tick(g.deadline);
        assert!(g.finished);
    }

    fn game(limit: Option<usize>, turns: bool) -> Match {
        let n = 32;
        let graph = (0..n).map(|i| vec![(i + n - 1) % n, (i + 1) % n]).collect();
        let mut g = Match::new(
            graph,
            0.1,
            [42; 32],
            Options {
                match_minutes: 20,
                turn_seconds: Some(30),
                mode: Mode::Territory,
                bordering: false,
                flood_limit: limit,
                turn_based: turns,
            },
            "a".into(),
        )
        .unwrap();
        g.join("b".into()).unwrap();
        g.start("a", 1000).unwrap();
        g
    }
    #[test]
    fn starts_are_only_revealed_when_bordering_is_enabled() {
        let g = game(Some(0), false);
        assert!(g.revealed.iter().all(|v| !v));
        assert!(g.owners.iter().all(Option::is_none));
        assert!(g.players.iter().all(|p| p.score == 0 && p.claims == 0));
        let mut bordered = g.clone();
        bordered.started = false;
        bordered.options.bordering = true;
        bordered.start("a", 1000).unwrap();
        assert_eq!(bordered.revealed.iter().filter(|v| **v).count(), 2);
        assert!(bordered.players.iter().all(|p| p.claims == 1));
        for mode in [Mode::Collaborative] {
            let mut shared = g.clone();
            shared.started = false;
            shared.options.mode = mode;
            shared.start("a", 1000).unwrap();
            assert!(shared.revealed.iter().all(|v| !v));
        }
    }

    #[test]
    fn hidden_cells_never_include_clues_or_voids() {
        let g = game(None, false);
        let value = serde_json::to_value(g.snapshot()).unwrap();
        for c in value["cells"].as_array().unwrap() {
            if c["revealed"] == false {
                assert!(c.get("isVoid").is_none());
                assert!(c.get("count").is_none());
            }
        }
        assert!(value.get("neighbors").is_none());
    }
    #[test]
    fn floods_reveal_but_do_not_claim_and_zero_disables() {
        for limit in [Some(0), Some(3), None] {
            let mut g = game(limit, false);
            let cell = (0..32)
                .find(|&i| !g.revealed[i] && !g.voids[i] && g.count(i) == 0)
                .unwrap();
            let before = g.revealed.iter().filter(|&&v| v).count();
            g.act("a", 1, Action::Select { cell }, 2000).unwrap();
            let revealed = g.revealed.iter().filter(|&&v| v).count() - before;
            assert_eq!(g.players[0].claims, 1);
            assert_eq!(g.owners.iter().filter(|o| **o == Some(0)).count(), 1);
            if let Some(limit) = limit {
                assert!(revealed <= 1 + limit);
            }
            if limit == Some(0) {
                assert_eq!(revealed, 1);
            }
            let revision = g.revision;
            g.act("a", 1, Action::Select { cell }, 2500).unwrap();
            assert_eq!(revision, g.revision);
        }
    }
    #[test]
    fn neutral_revealed_cells_remain_claimable() {
        let mut g = game(None, false);
        let cell = (0..32)
            .find(|&i| !g.revealed[i] && !g.voids[i] && g.count(i) == 0)
            .unwrap();
        g.act("a", 1, Action::Select { cell }, 2000).unwrap();
        let neutral = (0..32)
            .find(|&i| g.revealed[i] && !g.voids[i] && g.owners[i].is_none())
            .unwrap();
        g.act("b", 1, Action::Select { cell: neutral }, 3000)
            .unwrap();
        assert_eq!(g.owners[neutral], Some(1));
    }
    #[test]
    fn turns_cooldown_border_and_timeout_are_authoritative() {
        let mut g = game(Some(0), true);
        assert!(g.act("b", 1, Action::Pass, 2000).is_err());
        g.act("a", 1, Action::Pass, 2000).unwrap();
        g.act("b", 1, Action::Pass, 2100).unwrap();
        assert!(g.finished);
        let mut g = game(Some(0), false);
        g.options.bordering = true;
        let cell = (0..32)
            .find(|&i| !g.revealed[i] && !g.neighbors[i].iter().any(|&j| g.owners[j] == Some(0)))
            .unwrap();
        assert!(g.act("a", 1, Action::Select { cell }, 2000).is_err());
        assert!(!g.revealed[cell]);
        g.tick(g.deadline);
        assert!(g.finished);
    }
    #[test]
    fn collaborative_has_shared_reveals_no_ownership_and_no_turns() {
        let graph = (0..32).map(|i| vec![(i + 31) % 32, (i + 1) % 32]).collect();
        let mut g = Match::new(
            graph,
            0.1,
            [7; 32],
            Options {
                match_minutes: 20,
                turn_seconds: Some(30),
                mode: Mode::Collaborative,
                bordering: true,
                flood_limit: Some(0),
                turn_based: true,
            },
            "a".into(),
        )
        .unwrap();
        g.join("b".into()).unwrap();
        g.start("a", 1000).unwrap();
        assert!(!g.options.turn_based);
        assert!(!g.options.bordering);
        assert!(g.owners.iter().all(Option::is_none));
        // B may act first, and a late teammate sees the same persisted reveals.
        let cell = (0..32).find(|&i| !g.revealed[i] && !g.voids[i]).unwrap();
        g.act("b", 1, Action::Select { cell }, 2000).unwrap();
        assert!(g.revealed[cell]);
        assert!(g.act("a", 1, Action::Select { cell }, 2500).is_err());
        g.join("c".into()).unwrap();
        assert_eq!(g.players.iter().map(|p| p.score).sum::<i32>(), 1);
        let mut now = 3000;
        while !g.finished {
            let cell = (0..32).find(|&i| !g.revealed[i] && !g.voids[i]).unwrap();
            let sequence = g.players[0].sequence + 1;
            g.act("a", sequence, Action::Select { cell }, now).unwrap();
            now += 500;
        }
        assert!((0..32).all(|i| g.voids[i] || g.revealed[i]));
        assert!(g.owners.iter().all(Option::is_none));
    }

    #[test]
    fn private_state_roundtrip_preserves_snapshot() {
        let g = game(None, false);
        let restored: Match = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(g.snapshot()).unwrap(),
            serde_json::to_value(restored.snapshot()).unwrap()
        );
    }
    #[test]
    fn cumulative_turn_time_survives_restore_and_stops_at_finish() {
        let mut g = game(Some(0), true);
        g.act("a", 1, Action::Pass, 2500).unwrap();
        let mut g: Match = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
        g.act("a", 1, Action::Pass, 3000).unwrap(); // duplicate does not reset B's clock
        g.act("b", 1, Action::Pass, 5000).unwrap();
        g.tick(100000);
        assert_eq!(g.players[0].cumulative_ms, 1500);
        assert_eq!(g.players[1].cumulative_ms, 2500);
        assert_eq!(g.finished_at, Some(5000));
    }

    #[test]
    fn delayed_timeout_allocates_each_turn_without_counting_after_finish() {
        let mut g = game(Some(0), true);
        g.tick(999999);
        assert_eq!(g.players[0].cumulative_ms, 30000);
        assert_eq!(g.players[1].cumulative_ms, 30000);
        assert_eq!(g.finished_at, Some(61000));
    }

    #[test]
    fn simultaneous_time_starts_at_join_and_clips_to_expiration() {
        let mut g = game(Some(0), false);
        g.options.mode = Mode::Collaborative;
        g.tick(4000);
        g.join("c".into()).unwrap();
        g.tick(g.deadline + 999999);
        assert_eq!(g.players[0].cumulative_ms, g.deadline - 1000);
        assert_eq!(g.players[1].cumulative_ms, g.deadline - 1000);
        assert_eq!(g.players[2].cumulative_ms, g.deadline - 4000);
        assert_eq!(g.finished_at, Some(g.deadline));
    }

    #[test]
    fn unlimited_turn_time_clips_to_match_expiration() {
        let mut g = game(Some(0), true);
        g.options.turn_seconds = None;
        g.tick(g.deadline + 5000);
        assert_eq!(g.players[0].cumulative_ms, g.deadline - 1000);
        assert_eq!(g.players[1].cumulative_ms, 0);
    }
}
