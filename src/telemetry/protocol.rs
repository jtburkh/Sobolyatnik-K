use std::{error::Error, fmt};

use serde::Deserialize;

pub const PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:47777";

#[derive(Debug, Clone, PartialEq)]
pub enum TelemetryMessage {
    Snapshot(Snapshot),
    Gunshot(Gunshot),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Snapshot {
    pub version: u32,
    pub timestamp_ms: u64,
    pub player: PlayerSnapshot,
    #[serde(default)]
    pub map: MapSnapshot,
    #[serde(default)]
    pub ai: Vec<AiSnapshot>,
    #[serde(default)]
    pub loot: Vec<LootSnapshot>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MapSnapshot {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PlayerSnapshot {
    pub id: u64,
    pub position: [f64; 3],
    pub heading: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AiSnapshot {
    pub id: u64,
    pub position: [f64; 3],
    pub alive: bool,
    #[serde(default)]
    pub heading: Option<f64>,
    #[serde(default)]
    pub vision_range: Option<f64>,
    #[serde(default)]
    pub vision_half_angle: Option<f64>,
    #[serde(default)]
    pub player_visible: bool,
    #[serde(default)]
    pub boss: bool,
    #[serde(default)]
    pub faction: String,
    #[serde(default)]
    pub friendly: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LootSnapshot {
    pub id: u64,
    pub position: [f64; 3],
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub corpse: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Gunshot {
    pub version: u32,
    pub timestamp_ms: u64,
    pub shooter_id: u64,
    pub position: [f64; 3],
    #[serde(default)]
    pub map_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    MalformedJson(String),
    MissingType,
    UnsupportedType(String),
    UnsupportedVersion(u32),
    InvalidPayload(String),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedJson(error) => write!(formatter, "malformed JSON: {error}"),
            Self::MissingType => formatter.write_str("packet has no string type field"),
            Self::UnsupportedType(kind) => write!(formatter, "unsupported packet type: {kind}"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported protocol version: {version}")
            }
            Self::InvalidPayload(error) => write!(formatter, "invalid telemetry payload: {error}"),
        }
    }
}

impl Error for DecodeError {}

pub trait PacketDecoder: Send + Sync + 'static {
    fn decode(&self, packet: &[u8]) -> Result<TelemetryMessage, DecodeError>;
}

#[derive(Debug, Default)]
pub struct JsonDecoder;

impl PacketDecoder for JsonDecoder {
    fn decode(&self, packet: &[u8]) -> Result<TelemetryMessage, DecodeError> {
        let value: serde_json::Value = serde_json::from_slice(packet)
            .map_err(|error| DecodeError::MalformedJson(error.to_string()))?;
        let kind = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or(DecodeError::MissingType)?;
        let version = value
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .and_then(|version| u32::try_from(version).ok())
            .ok_or_else(|| DecodeError::InvalidPayload("version must be a u32".to_owned()))?;
        if version != PROTOCOL_VERSION {
            return Err(DecodeError::UnsupportedVersion(version));
        }

        match kind {
            "snapshot" => serde_json::from_value(value)
                .map(TelemetryMessage::Snapshot)
                .map_err(|error| DecodeError::InvalidPayload(error.to_string())),
            "gunshot" => serde_json::from_value(value)
                .map(TelemetryMessage::Gunshot)
                .map_err(|error| DecodeError::InvalidPayload(error.to_string())),
            other => Err(DecodeError::UnsupportedType(other.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DecodeError, JsonDecoder, PacketDecoder, TelemetryMessage};

    const SNAPSHOT: &str = r#"{
        "version": 1,
        "type": "snapshot",
        "timestamp_ms": 12345678,
        "map": {"id": "res://Scenes/Village.tscn", "name": "Village"},
        "player": {"id": 123, "position": [100.5, 3.2, -44.1], "heading": 217.3},
        "ai": [{"id": 501, "position": [145.2, 3.1, -81.9], "alive": true,
                  "heading": 90.0, "vision_range": 200.0,
                  "vision_half_angle": 60.0, "player_visible": true, "boss": true,
                  "faction": "Boss", "friendly": false}],
        "loot": [{"id": 801, "position": [110.0, 3.0, -40.0],
                   "name": "Military Crate", "locked": false, "corpse": false}]
    }"#;

    #[test]
    fn parses_snapshot_packets() {
        let decoded = JsonDecoder.decode(SNAPSHOT.as_bytes()).unwrap();
        let TelemetryMessage::Snapshot(snapshot) = decoded else {
            panic!("expected snapshot");
        };
        assert_eq!(snapshot.player.id, 123);
        assert_eq!(snapshot.player.position, [100.5, 3.2, -44.1]);
        assert_eq!(snapshot.map.name, "Village");
        assert_eq!(snapshot.map.id, "res://Scenes/Village.tscn");
        assert_eq!(snapshot.ai[0].id, 501);
        assert_eq!(snapshot.ai[0].heading, Some(90.0));
        assert!(snapshot.ai[0].boss);
        assert_eq!(snapshot.ai[0].faction, "Boss");
        assert!(!snapshot.ai[0].friendly);
        assert_eq!(snapshot.loot[0].name, "Military Crate");
    }

    #[test]
    fn accepts_legacy_snapshots_without_advanced_fields() {
        let legacy = r#"{
            "version": 1, "type": "snapshot", "timestamp_ms": 1,
            "player": {"id": 1, "position": [0, 0, 0], "heading": 0},
            "ai": [{"id": 2, "position": [1, 0, 1], "alive": true}]
        }"#;
        let TelemetryMessage::Snapshot(snapshot) = JsonDecoder.decode(legacy.as_bytes()).unwrap()
        else {
            panic!("expected snapshot");
        };
        assert!(snapshot.loot.is_empty());
        assert_eq!(snapshot.map, Default::default());
        assert_eq!(snapshot.ai[0].heading, None);
        assert!(!snapshot.ai[0].boss);
        assert!(snapshot.ai[0].faction.is_empty());
        assert!(!snapshot.ai[0].friendly);
    }

    #[test]
    fn accepts_scene_ids_on_gunshots_without_requiring_them_on_legacy_packets() {
        let legacy =
            br#"{"version":1,"type":"gunshot","timestamp_ms":1,"shooter_id":2,"position":[1,0,2]}"#;
        let TelemetryMessage::Gunshot(old) = JsonDecoder.decode(legacy).unwrap() else {
            panic!("expected gunshot");
        };
        assert!(old.map_id.is_empty());
        let scoped = br#"{"version":1,"type":"gunshot","timestamp_ms":1,"shooter_id":2,"position":[1,0,2],"map_id":"res://Scenes/Village.tscn"}"#;
        let TelemetryMessage::Gunshot(new) = JsonDecoder.decode(scoped).unwrap() else {
            panic!("expected gunshot");
        };
        assert_eq!(new.map_id, "res://Scenes/Village.tscn");
    }

    #[test]
    fn ignores_unknown_protocol_fields() {
        let packet = SNAPSHOT.replace(
            "\"timestamp_ms\": 12345678,",
            "\"timestamp_ms\": 12345678, \"future_field\": {\"enabled\": true},",
        );
        assert!(JsonDecoder.decode(packet.as_bytes()).is_ok());
    }

    #[test]
    fn reports_malformed_packets_without_panicking() {
        let error = JsonDecoder.decode(b"{not json").unwrap_err();
        assert!(matches!(error, DecodeError::MalformedJson(_)));
    }

    #[test]
    fn rejects_unknown_versions_and_message_types() {
        let version = SNAPSHOT.replace("\"version\": 1", "\"version\": 2");
        assert_eq!(
            JsonDecoder.decode(version.as_bytes()).unwrap_err(),
            DecodeError::UnsupportedVersion(2)
        );
        let kind = SNAPSHOT.replace("\"snapshot\"", "\"future\"");
        assert_eq!(
            JsonDecoder.decode(kind.as_bytes()).unwrap_err(),
            DecodeError::UnsupportedType("future".to_owned())
        );
    }
}
