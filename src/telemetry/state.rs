use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use super::protocol::{
    AiSnapshot, Gunshot, LootSnapshot, MapSnapshot, PlayerSnapshot, TelemetryMessage,
};

pub const AI_STALE_AFTER: Duration = Duration::from_secs(2);
pub const CONNECTION_STALE_AFTER: Duration = Duration::from_secs(2);
pub const SHOT_LIFETIME: Duration = Duration::from_secs(5);
pub const TRAIL_LIFETIME: Duration = Duration::from_secs(15);
const TRAIL_SAMPLE_INTERVAL: Duration = Duration::from_millis(400);
const TRAIL_MINIMUM_MOVEMENT: f64 = 0.35;
const TRAIL_TELEPORT_DISTANCE: f64 = 30.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Waiting,
    Live,
    Stale,
}

#[derive(Debug, Clone)]
pub struct TrackedMap {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct TrackedPlayer {
    pub id: u64,
    pub position: [f64; 3],
    pub heading: f64,
    pub last_seen: Instant,
}

#[derive(Debug, Clone)]
pub struct TrackedAi {
    pub id: u64,
    pub position: [f64; 3],
    pub alive: bool,
    pub heading: Option<f64>,
    pub vision_range: Option<f64>,
    pub vision_half_angle: Option<f64>,
    pub player_visible: bool,
    pub boss: bool,
    pub faction: String,
    pub friendly: bool,
    pub map_id: String,
    pub map_name: String,
    pub trail: VecDeque<TrailPoint>,
    pub last_seen: Instant,
}

#[derive(Debug, Clone)]
pub struct TrailPoint {
    pub position: [f64; 3],
    pub observed_at: Instant,
}

#[derive(Debug, Clone)]
pub struct TrackedLoot {
    pub position: [f64; 3],
    pub name: String,
    pub locked: bool,
    pub corpse: bool,
    pub last_seen: Instant,
}

#[derive(Debug, Clone)]
pub struct ShotEvent {
    pub shooter_id: u64,
    pub position: [f64; 3],
    pub occurred_at: Instant,
}

#[derive(Debug, Default)]
pub struct TelemetryState {
    pub current_map: Option<TrackedMap>,
    pub player: Option<TrackedPlayer>,
    pub summon_actions: Vec<String>,
    pub last_summon_snapshot: Option<Instant>,
    pub ai_entities: HashMap<u64, TrackedAi>,
    pub loot_containers: HashMap<u64, TrackedLoot>,
    pub shot_events: Vec<ShotEvent>,
    pub last_packet_time: Option<Instant>,
    pub packet_count: u64,
    pub malformed_packet_count: u64,
    pub last_socket_error: Option<String>,
}

impl TelemetryState {
    pub fn apply(&mut self, message: TelemetryMessage, received_at: Instant) {
        self.last_packet_time = Some(received_at);
        self.packet_count = self.packet_count.saturating_add(1);
        self.last_socket_error = None;
        match message {
            TelemetryMessage::Snapshot(snapshot) => {
                self.summon_actions = snapshot.summon_actions;
                self.last_summon_snapshot = Some(received_at);
                self.update_map(snapshot.map);
                self.update_player(snapshot.player, received_at);
                for entity in snapshot.ai {
                    self.update_ai(entity, received_at);
                }
                for container in snapshot.loot {
                    self.update_loot(container, received_at);
                }
            }
            TelemetryMessage::Gunshot(event) => self.add_shot(event, received_at),
        }
        self.prune(received_at);
    }

    pub fn prune(&mut self, now: Instant) {
        self.ai_entities.retain(|_, entity| {
            entity
                .trail
                .retain(|point| now.saturating_duration_since(point.observed_at) <= TRAIL_LIFETIME);
            now.saturating_duration_since(entity.last_seen) <= AI_STALE_AFTER
        });
        self.loot_containers
            .retain(|_, loot| now.saturating_duration_since(loot.last_seen) <= AI_STALE_AFTER);
        self.shot_events
            .retain(|event| now.saturating_duration_since(event.occurred_at) <= SHOT_LIFETIME);
    }

    pub fn connection_status(&self, now: Instant) -> ConnectionStatus {
        match self.last_packet_time {
            None => ConnectionStatus::Waiting,
            Some(last) if now.saturating_duration_since(last) <= CONNECTION_STALE_AFTER => {
                ConnectionStatus::Live
            }
            Some(_) => ConnectionStatus::Stale,
        }
    }

    pub fn can_summon(&self, action: &str, now: Instant) -> bool {
        self.connection_status(now) == ConnectionStatus::Live
            && self
                .last_summon_snapshot
                .is_some_and(|last| now.saturating_duration_since(last) <= CONNECTION_STALE_AFTER)
            && self
                .summon_actions
                .iter()
                .any(|available| available == action)
    }

    pub fn note_malformed_packet(&mut self) {
        self.malformed_packet_count = self.malformed_packet_count.saturating_add(1);
    }

    pub fn note_socket_error(&mut self, error: impl Into<String>) {
        self.last_socket_error = Some(error.into());
    }

    fn update_map(&mut self, map: MapSnapshot) {
        let id = map.id.trim().to_owned();
        let name = map.name.trim().to_owned();
        if self.current_map.as_ref().is_some_and(|old| old.id != id) {
            self.shot_events.clear();
        }
        if id.is_empty() && name.is_empty() {
            self.current_map = None;
            return;
        }
        self.current_map = Some(TrackedMap {
            name: if name.is_empty() {
                map_name_from_id(&id)
            } else {
                name
            },
            id,
        });
    }

    fn update_player(&mut self, player: PlayerSnapshot, received_at: Instant) {
        self.player = Some(TrackedPlayer {
            id: player.id,
            position: player.position,
            heading: player.heading.rem_euclid(360.0),
            last_seen: received_at,
        });
    }

    fn update_ai(&mut self, entity: AiSnapshot, received_at: Instant) {
        let (map_id, map_name) = self
            .current_map
            .as_ref()
            .map(|map| (map.id.clone(), map.name.clone()))
            .unwrap_or_default();
        let tracked = self
            .ai_entities
            .entry(entity.id)
            .or_insert_with(|| TrackedAi {
                id: entity.id,
                position: entity.position,
                alive: entity.alive,
                heading: entity.heading,
                vision_range: entity.vision_range,
                vision_half_angle: entity.vision_half_angle,
                player_visible: entity.player_visible,
                boss: entity.boss,
                faction: entity.faction.clone(),
                friendly: entity.friendly,
                map_id: map_id.clone(),
                map_name: map_name.clone(),
                trail: VecDeque::new(),
                last_seen: received_at,
            });
        let moved = horizontal_distance(tracked.position, entity.position);
        if moved > TRAIL_TELEPORT_DISTANCE {
            tracked.trail.clear();
        }
        let should_sample = tracked.trail.back().is_none_or(|last| {
            received_at.saturating_duration_since(last.observed_at) >= TRAIL_SAMPLE_INTERVAL
                && horizontal_distance(last.position, entity.position) >= TRAIL_MINIMUM_MOVEMENT
        });
        if should_sample {
            tracked.trail.push_back(TrailPoint {
                position: entity.position,
                observed_at: received_at,
            });
        }
        tracked.position = entity.position;
        tracked.alive = entity.alive;
        tracked.heading = entity
            .heading
            .filter(|heading| heading.is_finite())
            .map(|heading| heading.rem_euclid(360.0));
        tracked.vision_range = entity
            .vision_range
            .filter(|range| range.is_finite() && *range > 0.0);
        tracked.vision_half_angle = entity
            .vision_half_angle
            .filter(|angle| angle.is_finite() && *angle > 0.0 && *angle <= 180.0);
        tracked.player_visible = entity.player_visible;
        tracked.boss = entity.boss;
        tracked.faction = entity.faction;
        tracked.friendly = entity.friendly;
        tracked.map_id = map_id;
        tracked.map_name = map_name;
        tracked.last_seen = received_at;
    }

    fn update_loot(&mut self, container: LootSnapshot, received_at: Instant) {
        self.loot_containers.insert(
            container.id,
            TrackedLoot {
                position: container.position,
                name: container.name,
                locked: container.locked,
                corpse: container.corpse,
                last_seen: received_at,
            },
        );
    }

    fn add_shot(&mut self, event: Gunshot, received_at: Instant) {
        // Discard late packets from the previous scene rather than placing
        // their world-space markers on a different map. Old senders lacking
        // map_id keep their original five-second behavior.
        if !event.map_id.is_empty()
            && self
                .current_map
                .as_ref()
                .is_none_or(|map| map.id != event.map_id)
        {
            return;
        }
        self.shot_events.push(ShotEvent {
            shooter_id: event.shooter_id,
            position: event.position,
            occurred_at: received_at,
        });
    }
}

fn map_name_from_id(id: &str) -> String {
    id.rsplit('/')
        .next()
        .unwrap_or(id)
        .split('.')
        .next()
        .unwrap_or(id)
        .replace('_', " ")
}

fn horizontal_distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    (left[0] - right[0]).hypot(left[2] - right[2])
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{AI_STALE_AFTER, ConnectionStatus, SHOT_LIFETIME, TelemetryState};
    use crate::telemetry::protocol::{
        AiSnapshot, Gunshot, LootSnapshot, MapSnapshot, PlayerSnapshot, Snapshot, TelemetryMessage,
    };

    fn snapshot(ai: Vec<AiSnapshot>) -> TelemetryMessage {
        TelemetryMessage::Snapshot(Snapshot {
            version: 1,
            timestamp_ms: 10,
            summon_actions: vec![],
            map: MapSnapshot {
                id: "res://Scenes/Village.tscn".to_owned(),
                name: "Village".to_owned(),
            },
            player: PlayerSnapshot {
                id: 1,
                position: [0.0, 0.0, 0.0],
                heading: 0.0,
            },
            ai,
            loot: Vec::new(),
        })
    }

    fn ai(id: u64, position: [f64; 3]) -> AiSnapshot {
        AiSnapshot {
            id,
            position,
            alive: true,
            heading: Some(90.0),
            vision_range: Some(200.0),
            vision_half_angle: Some(60.0),
            player_visible: false,
            boss: false,
            faction: String::new(),
            friendly: false,
        }
    }

    #[test]
    fn summon_capabilities_require_a_fresh_compatible_snapshot() {
        let now = Instant::now();
        let mut state = TelemetryState::default();
        state.apply(snapshot(vec![]), now);
        assert!(!state.can_summon("spawn_airdrop", now)); // v1.1 telemetry has no receiver
        let TelemetryMessage::Snapshot(mut candidate) = snapshot(vec![]) else {
            unreachable!();
        };
        candidate.summon_actions = vec!["spawn_airdrop".to_owned(), "spawn_bogeyman".to_owned()];
        state.apply(TelemetryMessage::Snapshot(candidate), now);
        assert!(state.can_summon("spawn_airdrop", now));
        assert!(state.can_summon("spawn_bogeyman", now));
        assert!(!state.can_summon("spawn_punisher", now));
        state.apply(snapshot(vec![]), now + Duration::from_millis(100));
        assert!(!state.can_summon("spawn_airdrop", now + Duration::from_millis(100)));
        assert!(!state.can_summon("spawn_bogeyman", now + Duration::from_secs(3)));
    }

    #[test]
    fn removes_ai_only_after_the_stale_window() {
        let start = Instant::now();
        let mut state = TelemetryState::default();
        state.apply(snapshot(vec![ai(7, [1.0, 0.0, 2.0])]), start);
        assert_eq!(state.current_map.as_ref().unwrap().name, "Village");
        assert_eq!(state.ai_entities[&7].map_name, "Village");
        state.apply(snapshot(Vec::new()), start + Duration::from_secs(1));
        assert!(state.ai_entities.contains_key(&7));
        state.prune(start + AI_STALE_AFTER + Duration::from_millis(1));
        assert!(!state.ai_entities.contains_key(&7));
    }

    #[test]
    fn updates_nomad_allegiance_when_reputation_changes() {
        let start = Instant::now();
        let mut state = TelemetryState::default();
        let mut nomad = ai(7, [1.0, 0.0, 2.0]);
        nomad.faction = "Nomad".to_owned();
        nomad.friendly = true;
        state.apply(snapshot(vec![nomad.clone()]), start);
        assert_eq!(state.ai_entities[&7].faction, "Nomad");
        assert!(state.ai_entities[&7].friendly);
        nomad.friendly = false;
        state.apply(snapshot(vec![nomad]), start + Duration::from_millis(100));
        assert!(!state.ai_entities[&7].friendly);
    }

    #[test]
    fn samples_movement_trails_and_resets_after_a_teleport() {
        let start = Instant::now();
        let mut state = TelemetryState::default();
        state.apply(snapshot(vec![ai(7, [0.0, 1.0, 0.0])]), start);
        state.apply(
            snapshot(vec![ai(7, [1.0, 1.5, 0.0])]),
            start + Duration::from_millis(500),
        );
        assert_eq!(state.ai_entities[&7].trail.len(), 2);
        assert_eq!(state.ai_entities[&7].heading, Some(90.0));
        state.apply(
            snapshot(vec![ai(7, [100.0, 1.5, 100.0])]),
            start + Duration::from_secs(1),
        );
        assert_eq!(state.ai_entities[&7].trail.len(), 1);
    }

    #[test]
    fn tracks_and_expires_loot_containers() {
        let start = Instant::now();
        let mut state = TelemetryState::default();
        let TelemetryMessage::Snapshot(mut value) = snapshot(Vec::new()) else {
            unreachable!();
        };
        value.loot.push(LootSnapshot {
            id: 9,
            position: [4.0, 2.0, 5.0],
            name: "Crate".to_owned(),
            locked: true,
            corpse: false,
        });
        state.apply(TelemetryMessage::Snapshot(value), start);
        assert!(state.loot_containers[&9].locked);
        state.prune(start + AI_STALE_AFTER + Duration::from_millis(1));
        assert!(state.loot_containers.is_empty());
    }

    #[test]
    fn expires_gunshots_at_their_fixed_event_positions() {
        let start = Instant::now();
        let mut state = TelemetryState::default();
        state.apply(
            TelemetryMessage::Gunshot(Gunshot {
                version: 1,
                timestamp_ms: 20,
                shooter_id: 7,
                position: [12.0, 3.0, -8.0],
                map_id: String::new(),
            }),
            start,
        );
        assert_eq!(state.shot_events[0].position, [12.0, 3.0, -8.0]);
        state.prune(start + SHOT_LIFETIME + Duration::from_millis(1));
        assert!(state.shot_events.is_empty());
    }

    #[test]
    fn clears_shots_when_the_map_changes_and_rejects_late_packets() {
        let start = Instant::now();
        let mut state = TelemetryState::default();
        state.apply(snapshot(Vec::new()), start);
        let shot = Gunshot {
            version: 1,
            timestamp_ms: 20,
            shooter_id: 7,
            position: [12.0, 3.0, -8.0],
            map_id: "res://Scenes/Village.tscn".to_owned(),
        };
        state.apply(TelemetryMessage::Gunshot(shot.clone()), start);
        assert_eq!(state.shot_events.len(), 1);
        let TelemetryMessage::Snapshot(mut next) = snapshot(Vec::new()) else {
            unreachable!();
        };
        next.map.id = "res://Scenes/NewMap.tscn".to_owned();
        next.map.name = "New Map".to_owned();
        state.apply(
            TelemetryMessage::Snapshot(next),
            start + Duration::from_millis(200),
        );
        assert!(state.shot_events.is_empty());
        state.apply(
            TelemetryMessage::Gunshot(shot),
            start + Duration::from_millis(201),
        );
        assert!(state.shot_events.is_empty());
    }

    #[test]
    fn reports_waiting_live_and_stale_connection_states() {
        let start = Instant::now();
        let mut state = TelemetryState::default();
        assert_eq!(state.connection_status(start), ConnectionStatus::Waiting);
        state.apply(snapshot(Vec::new()), start);
        assert_eq!(state.connection_status(start), ConnectionStatus::Live);
        assert_eq!(
            state.connection_status(start + Duration::from_secs(3)),
            ConnectionStatus::Stale
        );
    }
}
