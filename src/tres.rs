use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::Write,
    ops::Range,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use regex::Regex;

use crate::{
    catalog::{Catalog, CatalogItem, WeaponStatsBook},
    grid::{CELL_SIZE, ValidationReport, find_free_slot, validate},
};

const SLOT_DATA_PATH: &str = "res://Scripts/SlotData.gd";
const ITEM_DATA_PATH: &str = "res://Scripts/ItemData.gd";

#[derive(Clone, Debug)]
pub struct InventoryItem {
    pub resource_id: String,
    pub path: String,
    pub x: i32,
    pub y: i32,
    pub rotated: bool,
    pub amount: i64,
    pub condition: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EquipmentItem {
    /// Sub-resource id in Character.tres holding this slot item's data — the
    /// write target for amount/condition edits.
    pub resource_id: String,
    pub path: String,
    pub condition: f64,
    pub amount: i64,
    pub chamber: bool,
    /// Equipped contents (attachments, magazines, ...), resolved to paths.
    pub nested: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EquipmentSlot {
    pub slot: String,
    /// None when the slot carries no equipped item.
    pub item: Option<EquipmentItem>,
}

pub fn validate_equipment_values(
    slots: &[EquipmentSlot],
    catalog: &Catalog,
    weapon_stats: &WeaponStatsBook,
) -> Vec<String> {
    let mut errors = Vec::new();
    for item in slots.iter().filter_map(|slot| slot.item.as_ref()) {
        let meta = catalog.get(&item.path);
        let name = meta.map_or(item.path.as_str(), |entry| entry.name.as_str());
        if item.amount < 0 {
            errors.push(format!("{name} has a negative amount"));
        }
        let has_amount = meta.is_some_and(|entry| {
            entry.show_amount || entry.stackable || entry.category == "Weapons"
        });
        let maximum = weapon_stats
            .get(&item.path)
            .and_then(|stats| stats.magazine_size)
            .or_else(|| meta.and_then(|entry| entry.max_amount.map(|value| value as f64)));
        if has_amount && maximum.is_some_and(|max| item.amount as f64 > max) {
            errors.push(format!(
                "{name} amount {} exceeds maximum {}",
                item.amount,
                maximum.unwrap_or_default()
            ));
        }

        let has_condition = meta.is_some_and(|entry| {
            entry.show_condition
                || matches!(entry.category.as_str(), "Weapons" | "Armor" | "Helmets")
                || (entry.category == "Rigs"
                    && item.nested.iter().any(|nested| {
                        catalog
                            .get(nested)
                            .is_some_and(|attachment| attachment.category == "Armor")
                    }))
        });
        if has_condition && !(0.0..=100.0).contains(&item.condition) {
            errors.push(format!(
                "{name} condition {} is outside 0–100",
                format_number(item.condition)
            ));
        }
    }
    errors
}

#[derive(Clone, Debug)]
struct PropertySpan {
    value: Range<usize>,
}

#[derive(Clone, Debug)]
struct ExternalResource {
    id: String,
    path: String,
    line: Range<usize>,
}

#[derive(Clone, Debug)]
struct SubResource {
    id: String,
    span: Range<usize>,
    properties: HashMap<String, PropertySpan>,
}

#[derive(Debug)]
struct ParsedTres {
    externals: Vec<ExternalResource>,
    subs: Vec<SubResource>,
    root_header: usize,
    root_properties: HashMap<String, PropertySpan>,
}

#[derive(Debug)]
pub struct CharacterDocument {
    path: PathBuf,
    text: String,
    original_text: String,
}

/// How long the character has been alive in the campaign, plus the world
/// conditions the game records next to it in `World.tres`.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldState {
    pub day: i64,
    pub minutes: f64,
    pub season: Option<i64>,
    pub difficulty: Option<i64>,
    pub weather: Option<String>,
}

impl WorldState {
    /// Game minutes since midnight -> 24h clock string, e.g. 245.67 -> "04:05".
    pub fn clock(&self) -> String {
        let total = self.minutes.rem_euclid(1440.0);
        format!("{:02}:{:02}", (total / 60.0) as u32, (total % 60.0) as u32)
    }
}

fn parse_world_text(text: &str) -> Option<WorldState> {
    let parsed = parse_tres(text).ok()?;
    let root_value = |key: &str| {
        parsed
            .root_properties
            .get(key)
            .map(|prop| text[prop.value.clone()].trim())
    };
    let minutes: f64 = root_value("time")?.parse().ok()?;
    if !minutes.is_finite() {
        return None;
    }
    Some(WorldState {
        day: root_value("day")?.parse().ok()?,
        minutes,
        season: root_value("season").and_then(|value| value.parse().ok()),
        difficulty: root_value("difficulty").and_then(|value| value.parse().ok()),
        weather: root_value("weather").map(|value| value.trim_matches('"').to_owned()),
    })
}

impl CharacterDocument {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let text = fs::read_to_string(&path)
            .with_context(|| format!("could not read {}", path.display()))?;
        ensure_character_file(&text)?;
        parse_tres(&text)?;
        Ok(Self {
            path,
            original_text: text.clone(),
            text,
        })
    }

    #[cfg(test)]
    pub fn from_text(text: &str) -> Result<Self> {
        ensure_character_file(text)?;
        parse_tres(text)?;
        Ok(Self {
            path: PathBuf::from("Character.tres"),
            text: text.to_owned(),
            original_text: text.to_owned(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The campaign's mission clock: `World.tres` is saved in the same
    /// directory as the character file by the game itself.
    pub fn world_state(&self) -> Option<WorldState> {
        self.world_state_result().ok().flatten()
    }

    pub fn world_state_result(&self) -> Result<Option<WorldState>> {
        let Some(parent) = self.path.parent() else {
            return Ok(None);
        };
        let path = parent.join("World.tres");
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).with_context(|| format!("could not read {}", path.display()));
            }
        };
        parse_world_text(&text)
            .map(Some)
            .ok_or_else(|| anyhow!("{} has invalid root day/time data", path.display()))
    }

    pub fn is_modified(&self) -> bool {
        self.text != self.original_text
    }

    pub fn inventory(&self) -> Result<Vec<InventoryItem>> {
        inventory_from(&self.text)
    }

    /// Parse the character's equipped slots (weapons, armor, clothing, and
    /// utilities). Edits target individual slot sub-resources by ID.
    pub fn equipment(&self) -> Result<Vec<EquipmentSlot>> {
        equipment_from(&self.text)
    }

    pub fn validation(&self, catalog: &Catalog) -> Result<ValidationReport> {
        Ok(validate(&self.inventory()?, catalog))
    }

    pub fn stat(&self, key: &str) -> Option<String> {
        let parsed = parse_tres(&self.text).ok()?;
        let prop = parsed.root_properties.get(key)?;
        Some(self.text[prop.value.clone()].to_owned())
    }

    pub fn vital_validation_errors(&self) -> Vec<String> {
        ["health", "energy", "hydration", "temperature", "mental"]
            .into_iter()
            .filter_map(|key| match self.stat(key) {
                Some(raw) => match raw.parse::<f64>() {
                    Ok(value) if value.is_finite() && (0.0..=100.0).contains(&value) => None,
                    _ => Some(format!("{key} must be a number from 0 to 100")),
                },
                None => Some(format!("missing root {key} property")),
            })
            .collect()
    }

    pub fn set_vital(&mut self, key: &str, value: f64) -> Result<()> {
        if !matches!(
            key,
            "health" | "energy" | "hydration" | "temperature" | "mental"
        ) {
            bail!("{key} is not an editable character vital");
        }
        if !value.is_finite() || !(0.0..=100.0).contains(&value) {
            bail!("{key} must be between 0 and 100");
        }
        let parsed = parse_tres(&self.text)?;
        let property = parsed
            .root_properties
            .get(key)
            .ok_or_else(|| anyhow!("missing root {key} property"))?;
        self.text
            .replace_range(property.value.clone(), &format_number(value));
        Ok(())
    }

    pub fn add_item(&mut self, item: &CatalogItem, catalog: &Catalog) -> Result<InventoryItem> {
        let inventory = self.inventory()?;
        let report = validate(&inventory, catalog);
        if !report.is_valid() {
            bail!(
                "existing inventory is invalid; fix it before adding items: {}",
                report.errors[0]
            );
        }
        let placement = find_free_slot(&inventory, catalog, &item.path)
            .ok_or_else(|| anyhow!("no collision-free space remains for {}", item.name))?;

        let slot_data_id = self.ensure_external(SLOT_DATA_PATH, "Script")?;
        let item_data_id = self.ensure_external(ITEM_DATA_PATH, "Script")?;
        let item_ext_id = self.ensure_external(&item.path, "Resource")?;
        let resource_id = self.new_subresource_id()?;

        // SlotData defaults to pristine even for items whose gameplay UI does
        // not use condition. Preserve that convention so the stored value is
        // truthful if a later game build starts using it.
        let condition = 100;
        let block = format!(
            "[sub_resource type=\"Resource\" id=\"{resource_id}\"]\n\
script = ExtResource(\"{slot_data_id}\")\n\
itemData = ExtResource(\"{item_ext_id}\")\n\
nested = Array[ExtResource(\"{item_data_id}\")]([])\n\
storage = Array[ExtResource(\"{slot_data_id}\")]([])\n\
condition = {condition}\n\
amount = {}\n\
position = 0\n\
mode = 1\n\
zoom = 1\n\
chamber = false\n\
casing = false\n\
state = \"\"\n\
gridPosition = Vector2({}, {})\n\
gridRotated = {}\n\
slot = \"\"\n\n",
            item.default_amount,
            placement.col as i32 * CELL_SIZE,
            placement.row as i32 * CELL_SIZE,
            placement.rotated
        );

        let parsed = parse_tres(&self.text)?;
        self.text.insert_str(parsed.root_header, &block);
        self.append_inventory_ref(&resource_id)?;

        let added = self
            .inventory()?
            .into_iter()
            .find(|entry| entry.resource_id == resource_id)
            .ok_or_else(|| anyhow!("internal error: added item was not readable"))?;
        let final_report = self.validation(catalog)?;
        if !final_report.is_valid() {
            bail!(
                "internal error: placement validation failed: {}",
                final_report.errors[0]
            );
        }
        Ok(added)
    }

    pub fn remove_item(&mut self, resource_id: &str) -> Result<()> {
        let parsed = parse_tres(&self.text)?;
        let inventory = inventory_refs(&self.text, &parsed)?;
        if !inventory.iter().any(|id| id == resource_id) {
            bail!("{resource_id} is not in the character inventory");
        }
        let retained: Vec<String> = inventory
            .into_iter()
            .filter(|id| id != resource_id)
            .collect();
        self.replace_inventory_refs(&retained)?;

        let parsed = parse_tres(&self.text)?;
        let sub = parsed
            .subs
            .iter()
            .find(|sub| sub.id == resource_id)
            .ok_or_else(|| anyhow!("missing sub_resource {resource_id}"))?;
        self.text.replace_range(sub.span.clone(), "");
        Ok(())
    }

    pub fn set_amount(&mut self, resource_id: &str, amount: i64) -> Result<()> {
        if amount < 0 {
            bail!("amount cannot be negative");
        }
        self.replace_sub_property(resource_id, "amount", &amount.to_string())
    }

    pub fn set_condition(&mut self, resource_id: &str, condition: f64) -> Result<()> {
        if !(0.0..=100.0).contains(&condition) {
            bail!("condition must be between 0 and 100");
        }
        self.replace_sub_property(resource_id, "condition", &format_number(condition))
    }

    pub fn reload(&mut self) -> Result<()> {
        let replacement = Self::load(&self.path)?;
        *self = replacement;
        Ok(())
    }

    pub fn save(&mut self) -> Result<PathBuf> {
        let on_disk = fs::read_to_string(&self.path)
            .with_context(|| format!("could not re-read {} before saving", self.path.display()))?;
        if on_disk != self.original_text {
            bail!("the save changed on disk; reload it instead of overwriting game changes");
        }

        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        let filename = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Character.tres");
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let backup = parent.join(format!("{filename}.rtvbak.{stamp}"));
        let temporary = parent.join(format!(".{filename}.{stamp}.rtvtmp"));

        let mut temp = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("could not create {}", temporary.display()))?;
        temp.write_all(self.text.as_bytes())?;
        temp.sync_all()?;
        drop(temp);

        if let Err(error) = fs::rename(&self.path, &backup) {
            let _ = fs::remove_file(&temporary);
            return Err(error).context("could not create the save backup");
        }
        if let Err(error) = fs::rename(&temporary, &self.path) {
            let _ = fs::rename(&backup, &self.path);
            let _ = fs::remove_file(&temporary);
            return Err(error).context("could not install the updated save; original was restored");
        }

        self.original_text = self.text.clone();
        Ok(backup)
    }

    fn ensure_external(&mut self, path: &str, resource_type: &str) -> Result<String> {
        let parsed = parse_tres(&self.text)?;
        if let Some(existing) = parsed.externals.iter().find(|ext| ext.path == path) {
            return Ok(existing.id.clone());
        }

        let used: HashSet<String> = parsed.externals.iter().map(|ext| ext.id.clone()).collect();
        let mut next = parsed
            .externals
            .iter()
            .filter_map(|ext| ext.id.parse::<u64>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        while used.contains(&next.to_string()) {
            next += 1;
        }
        let id = next.to_string();
        let line = format!("[ext_resource type=\"{resource_type}\" path=\"{path}\" id=\"{id}\"]\n");
        let insert_at = parsed
            .externals
            .last()
            .map(|ext| ext.line.end)
            .unwrap_or_else(|| first_line_end(&self.text));
        self.text.insert_str(insert_at, &line);
        Ok(id)
    }

    fn new_subresource_id(&self) -> Result<String> {
        let parsed = parse_tres(&self.text)?;
        let used: HashSet<&str> = parsed.subs.iter().map(|sub| sub.id.as_str()).collect();
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        for offset in 0..10_000 {
            let suffix = base36(seed.wrapping_add(offset));
            let suffix = &suffix[suffix.len().saturating_sub(5)..];
            let id = format!("Resource_{suffix:0>5}");
            if !used.contains(id.as_str()) {
                return Ok(id);
            }
        }
        bail!("could not generate a unique sub_resource ID")
    }

    fn append_inventory_ref(&mut self, resource_id: &str) -> Result<()> {
        let parsed = parse_tres(&self.text)?;
        let mut refs = inventory_refs(&self.text, &parsed)?;
        refs.push(resource_id.to_owned());
        self.replace_inventory_refs(&refs)
    }

    fn replace_inventory_refs(&mut self, refs: &[String]) -> Result<()> {
        let parsed = parse_tres(&self.text)?;
        let prop = parsed
            .root_properties
            .get("inventory")
            .ok_or_else(|| anyhow!("missing root inventory property"))?;
        let old = &self.text[prop.value.clone()];
        let (array_type, _) = parse_slot_refs(old)?;
        let refs = refs
            .iter()
            .map(|id| format!("SubResource(\"{id}\")"))
            .collect::<Vec<_>>()
            .join(", ");
        let replacement = format!("Array[ExtResource(\"{array_type}\")]([{refs}])");
        self.text.replace_range(prop.value.clone(), &replacement);
        Ok(())
    }

    fn replace_sub_property(&mut self, resource_id: &str, key: &str, value: &str) -> Result<()> {
        let parsed = parse_tres(&self.text)?;
        let sub = parsed
            .subs
            .iter()
            .find(|sub| sub.id == resource_id)
            .ok_or_else(|| anyhow!("missing sub_resource {resource_id}"))?;
        let property = sub
            .properties
            .get(key)
            .ok_or_else(|| anyhow!("{resource_id} has no {key} property"))?;
        self.text.replace_range(property.value.clone(), value);
        Ok(())
    }
}

fn ensure_character_file(text: &str) -> Result<()> {
    let first = text.lines().next().unwrap_or_default();
    if !first.contains("[gd_resource") || !first.contains("script_class=\"CharacterSave\"") {
        bail!("file is not a CharacterSave Godot text resource");
    }
    Ok(())
}

fn parse_tres(text: &str) -> Result<ParsedTres> {
    let lines = line_ranges(text);
    let mut externals = Vec::new();
    let mut headers = Vec::new();
    let mut root_header = None;

    for range in &lines {
        let line = text[range.clone()].trim();
        if line.starts_with("[ext_resource") {
            externals.push(ExternalResource {
                id: attr(line, "id").ok_or_else(|| anyhow!("ext_resource without id"))?,
                path: attr(line, "path").ok_or_else(|| anyhow!("ext_resource without path"))?,
                line: range.clone(),
            });
        } else if line.starts_with("[sub_resource") {
            headers.push((
                range.start,
                attr(line, "id").ok_or_else(|| anyhow!("sub_resource without id"))?,
            ));
        } else if line == "[resource]" {
            root_header = Some(range.start);
        }
    }

    let root_header = root_header.ok_or_else(|| anyhow!("missing [resource] section"))?;
    let mut external_ids = HashSet::new();
    for external in &externals {
        if !external_ids.insert(external.id.as_str()) {
            bail!("duplicate ext_resource ID {}", external.id);
        }
    }
    let mut sub_ids = HashSet::new();
    for (_, id) in &headers {
        if !sub_ids.insert(id.as_str()) {
            bail!("duplicate sub_resource ID {id}");
        }
    }
    let mut subs = Vec::new();
    for (index, (start, id)) in headers.iter().enumerate() {
        let end = headers
            .get(index + 1)
            .map(|next| next.0)
            .unwrap_or(root_header);
        subs.push(SubResource {
            id: id.clone(),
            span: *start..end,
            properties: properties_in(text, *start..end),
        });
    }
    let root_properties = properties_in(text, root_header..text.len());
    Ok(ParsedTres {
        externals,
        subs,
        root_header,
        root_properties,
    })
}

fn inventory_from(text: &str) -> Result<Vec<InventoryItem>> {
    let parsed = parse_tres(text)?;
    let ext_paths: HashMap<&str, &str> = parsed
        .externals
        .iter()
        .map(|ext| (ext.id.as_str(), ext.path.as_str()))
        .collect();
    let sub_by_id: HashMap<&str, &SubResource> = parsed
        .subs
        .iter()
        .map(|sub| (sub.id.as_str(), sub))
        .collect();
    let ext_ref = Regex::new(r#"^ExtResource\(\"([^\"]+)\"\)$"#)?;
    let vector = Regex::new(r"^Vector2\(([-+\d.eE]+),\s*([-+\d.eE]+)\)$")?;
    let mut items = Vec::new();

    for id in inventory_refs(text, &parsed)? {
        let sub = sub_by_id
            .get(id.as_str())
            .ok_or_else(|| anyhow!("inventory references missing sub_resource {id}"))?;
        let item_data = property_value(text, sub, "itemData")?;
        let ext_id = ext_ref
            .captures(item_data)
            .and_then(|captures| captures.get(1))
            .map(|value| value.as_str())
            .ok_or_else(|| anyhow!("{id} has invalid itemData: {item_data}"))?;
        let path = ext_paths
            .get(ext_id)
            .ok_or_else(|| anyhow!("{id} references missing ExtResource {ext_id}"))?;
        let position = property_value(text, sub, "gridPosition")?;
        let captures = vector
            .captures(position)
            .ok_or_else(|| anyhow!("{id} has invalid gridPosition: {position}"))?;
        let x = parse_grid_coordinate(&captures[1], &id)?;
        let y = parse_grid_coordinate(&captures[2], &id)?;
        let rotated = sub
            .properties
            .get("gridRotated")
            .map(|property| &text[property.value.clone()] == "true")
            .unwrap_or(false);
        let amount = optional_property_value(text, sub, "amount")
            .unwrap_or("0")
            .parse::<i64>()
            .with_context(|| format!("{id} has invalid amount"))?;
        let condition = optional_property_value(text, sub, "condition")
            .unwrap_or("0")
            .parse::<f64>()
            .with_context(|| format!("{id} has invalid condition"))?;
        items.push(InventoryItem {
            resource_id: id,
            path: (*path).to_owned(),
            x,
            y,
            rotated,
            amount,
            condition,
        });
    }
    Ok(items)
}

fn inventory_refs(text: &str, parsed: &ParsedTres) -> Result<Vec<String>> {
    let prop = parsed
        .root_properties
        .get("inventory")
        .ok_or_else(|| anyhow!("missing root inventory property"))?;
    let value = &text[prop.value.clone()];
    Ok(parse_slot_refs(value)?.1)
}

fn equipment_from(text: &str) -> Result<Vec<EquipmentSlot>> {
    let parsed = parse_tres(text)?;
    let prop = parsed
        .root_properties
        .get("equipment")
        .ok_or_else(|| anyhow!("missing root equipment property"))?;
    let value = &text[prop.value.clone()];
    let (_, ids) = parse_slot_refs(value)?;

    let ext_paths: HashMap<&str, &str> = parsed
        .externals
        .iter()
        .map(|ext| (ext.id.as_str(), ext.path.as_str()))
        .collect();
    let sub_by_id: HashMap<&str, &SubResource> = parsed
        .subs
        .iter()
        .map(|sub| (sub.id.as_str(), sub))
        .collect();
    let ext_ref = Regex::new(r#"^ExtResource\(\"([^"]+)\"\)$"#)?;
    let nested_array = Regex::new(r#"^Array\[ExtResource\(\"[^"]+\"\)\]\(\[([^\]]*)\]\)$"#)?;

    let mut slots = Vec::with_capacity(ids.len());
    for id in &ids {
        let sub = sub_by_id
            .get(id.as_str())
            .ok_or_else(|| anyhow!("equipment references missing sub_resource {id}"))?;
        let name = optional_property_value(text, sub, "slot")
            .map(|raw| raw.trim_matches('\"').to_owned())
            .unwrap_or_else(|| id.clone());

        let item = match optional_property_value(text, sub, "itemData") {
            Some(raw) => {
                let ext_id = ext_ref
                    .captures(raw)
                    .and_then(|captures| captures.get(1))
                    .map(|value| value.as_str())
                    .ok_or_else(|| anyhow!("{id} has invalid itemData: {raw}"))?;
                let path = ext_paths
                    .get(ext_id)
                    .ok_or_else(|| anyhow!("{id} references missing ExtResource {ext_id}"))?;
                let nested = match optional_property_value(text, sub, "nested") {
                    None => Vec::new(),
                    Some(raw) => {
                        let contents = nested_array
                            .captures(raw)
                            .and_then(|captures| captures.get(1))
                            .map(|value| value.as_str().trim())
                            .ok_or_else(|| anyhow!("{id} has invalid nested array: {raw}"))?;
                        if contents.is_empty() {
                            Vec::new()
                        } else {
                            let mut nested = Vec::new();
                            for entry in contents.split(',') {
                                let entry = entry.trim();
                                let nested_id = ext_ref
                                    .captures(entry)
                                    .and_then(|captures| captures.get(1))
                                    .map(|value| value.as_str())
                                    .ok_or_else(|| {
                                        anyhow!("{id} has invalid nested entry: {entry}")
                                    })?;
                                let nested_path = ext_paths.get(nested_id).ok_or_else(|| {
                                    anyhow!("{id} nested item references missing ExtResource {nested_id}")
                                })?;
                                nested.push((*nested_path).to_owned());
                            }
                            nested
                        }
                    }
                };
                let condition = optional_property_value(text, sub, "condition")
                    .unwrap_or("0")
                    .parse::<f64>()
                    .with_context(|| format!("{id} has invalid condition"))?;
                let amount = optional_property_value(text, sub, "amount")
                    .unwrap_or("0")
                    .parse::<i64>()
                    .with_context(|| format!("{id} has invalid amount"))?;
                let chamber = sub
                    .properties
                    .get("chamber")
                    .map(|property| &text[property.value.clone()] == "true")
                    .unwrap_or(false);
                Some(EquipmentItem {
                    resource_id: id.clone(),
                    path: path.to_string(),
                    condition,
                    amount,
                    chamber,
                    nested,
                })
            }
            None => None,
        };

        slots.push(EquipmentSlot { slot: name, item });
    }
    Ok(slots)
}

fn parse_slot_refs(value: &str) -> Result<(String, Vec<String>)> {
    let array = Regex::new(r#"^Array\[ExtResource\(\"([^\"]+)\"\)\]\(\[(.*)\]\)$"#)?;
    let captures = array
        .captures(value)
        .ok_or_else(|| anyhow!("unsupported inventory array format: {value}"))?;
    let array_type = captures[1].to_owned();
    let contents = captures[2].trim();
    if contents.is_empty() {
        return Ok((array_type, Vec::new()));
    }

    let reference = Regex::new(r#"^SubResource\(\"([^\"]+)\"\)$"#)?;
    let mut refs = Vec::new();
    for entry in contents.split(',') {
        let entry = entry.trim();
        let id = reference
            .captures(entry)
            .and_then(|captures| captures.get(1))
            .map(|matched| matched.as_str())
            .ok_or_else(|| anyhow!("unsupported inventory entry: {entry}"))?;
        refs.push(id.to_owned());
    }
    Ok((array_type, refs))
}

fn property_value<'a>(text: &'a str, sub: &SubResource, key: &str) -> Result<&'a str> {
    optional_property_value(text, sub, key)
        .ok_or_else(|| anyhow!("{} has no {key} property", sub.id))
}

fn optional_property_value<'a>(text: &'a str, sub: &SubResource, key: &str) -> Option<&'a str> {
    sub.properties
        .get(key)
        .map(|property| &text[property.value.clone()])
}

fn properties_in(text: &str, span: Range<usize>) -> HashMap<String, PropertySpan> {
    let mut properties = HashMap::new();
    for line_range in line_ranges(&text[span.clone()]) {
        let absolute = span.start + line_range.start..span.start + line_range.end;
        let raw = &text[absolute.clone()];
        let no_newline = raw.trim_end_matches(['\r', '\n']);
        let Some(equal) = no_newline.find('=') else {
            continue;
        };
        let key = no_newline[..equal].trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|character| character == '_' || character.is_ascii_alphanumeric())
        {
            continue;
        }
        let value_part = &no_newline[equal + 1..];
        let leading = value_part.len() - value_part.trim_start().len();
        let trailing = value_part.trim_end().len();
        let value_start = absolute.start + equal + 1 + leading;
        let value_end = absolute.start + equal + 1 + trailing;
        properties.insert(
            key.to_owned(),
            PropertySpan {
                value: value_start..value_end,
            },
        );
    }
    properties
}

fn line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut result = Vec::new();
    let mut start = 0;
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            result.push(start..index + 1);
            start = index + 1;
        }
    }
    if start < text.len() {
        result.push(start..text.len());
    }
    result
}

fn attr(line: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = line.find(&needle)? + needle.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_owned())
}

fn parse_grid_coordinate(raw: &str, id: &str) -> Result<i32> {
    let value = raw
        .parse::<f64>()
        .with_context(|| format!("{id} has invalid grid coordinate {raw}"))?;
    if !value.is_finite() || value < i32::MIN as f64 || value > i32::MAX as f64 {
        bail!("{id} has out-of-range grid coordinate {raw}");
    }
    if value.fract().abs() > f64::EPSILON {
        bail!("{id} has fractional grid coordinate {raw}");
    }
    Ok(value as i32)
}

fn first_line_end(text: &str) -> usize {
    text.find('\n').map(|index| index + 1).unwrap_or(text.len())
}

fn base36(mut value: u64) -> String {
    let mut output = Vec::new();
    loop {
        let digit = (value % 36) as u8;
        output.push(if digit < 10 {
            b'0' + digit
        } else {
            b'a' + digit - 10
        });
        value /= 36;
        if value == 0 {
            break;
        }
    }
    output.reverse();
    String::from_utf8(output).expect("base36 is ASCII")
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: &str = include_str!("../tests/fixtures/empty_character.tres");

    const EQUIPPED: &str = r#"[gd_resource type="Resource" script_class="CharacterSave" format=3]

[ext_resource type="Script" path="res://Scripts/SlotData.gd" id="1"]
[ext_resource type="Script" path="res://Scripts/ItemData.gd" id="2"]
[ext_resource type="Resource" path="res://Items/Weapons/M78/M78.tres" id="3"]
[ext_resource type="Resource" path="res://Items/Attachments/Leopard/Leopard.tres" id="4"]
[ext_resource type="Script" path="res://Scripts/CharacterSave.gd" id="5"]

[sub_resource type="Resource" id="Resource_eqpri"]
script = ExtResource("1")
itemData = ExtResource("3")
nested = Array[ExtResource("2")]([ExtResource("4")])
storage = Array[ExtResource("1")]([])
condition = 93
amount = 18
position = 0
mode = 1
zoom = 1
chamber = true
casing = false
state = ""
gridPosition = Vector2(0, 0)
gridRotated = false
slot = "Primary"

[sub_resource type="Resource" id="Resource_eqsec"]
script = ExtResource("1")
condition = 100
amount = 0
position = 0
mode = 1
zoom = 1
chamber = false
casing = false
state = ""
gridPosition = Vector2(0, 0)
gridRotated = false
slot = "Secondary"

[resource]
script = ExtResource("5")
health = 100.0
energy = 100.0
hydration = 100.0
temperature = 100.0
mental = 100.0
inventory = Array[ExtResource("1")]()
equipment = Array[ExtResource("1")]([SubResource("Resource_eqpri"), SubResource("Resource_eqsec")])
catalog = Array[ExtResource("1")]()
"#;

    #[test]
    fn edits_only_whitelisted_root_vitals() {
        let mut doc = CharacterDocument::from_text(EQUIPPED).unwrap();
        doc.set_vital("health", 72.5).unwrap();
        assert_eq!(doc.stat("health").as_deref(), Some("72.5"));
        assert!(doc.is_modified());
        assert!(doc.set_vital("health", 101.0).is_err());
        assert!(doc.set_vital("inventory", 1.0).is_err());
        assert!(doc.vital_validation_errors().is_empty());

        let invalid = EQUIPPED.replace("health = 100.0", "health = 101.0");
        let invalid = CharacterDocument::from_text(&invalid).unwrap();
        assert!(invalid.vital_validation_errors()[0].contains("health"));
    }

    #[test]
    fn parses_equipped_slots() {
        let doc = CharacterDocument::from_text(EQUIPPED).unwrap();
        let slots = doc.equipment().unwrap();

        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].slot, "Primary");
        let item = slots[0].item.as_ref().unwrap();
        assert_eq!(item.path, "res://Items/Weapons/M78/M78.tres");
        assert_eq!(item.condition, 93.0);
        assert_eq!(item.amount, 18);
        assert!(item.chamber);
        assert_eq!(
            item.nested,
            vec!["res://Items/Attachments/Leopard/Leopard.tres"]
        );

        assert_eq!(slots[1].slot, "Secondary");
        assert!(slots[1].item.is_none());
    }

    #[test]
    fn rejects_malformed_or_missing_nested_equipment_references() {
        let malformed = EQUIPPED.replace(
            "nested = Array[ExtResource(\"2\")]([ExtResource(\"4\")])",
            "nested = Array[ExtResource(\"2\")]([not_a_resource])",
        );
        let doc = CharacterDocument::from_text(&malformed).unwrap();
        assert!(
            doc.equipment()
                .unwrap_err()
                .to_string()
                .contains("invalid nested entry")
        );

        let missing = EQUIPPED.replace(
            "nested = Array[ExtResource(\"2\")]([ExtResource(\"4\")])",
            "nested = Array[ExtResource(\"2\")]([ExtResource(\"99\")])",
        );
        let doc = CharacterDocument::from_text(&missing).unwrap();
        assert!(
            doc.equipment()
                .unwrap_err()
                .to_string()
                .contains("missing ExtResource 99")
        );
    }

    #[test]
    fn equipment_validation_uses_weapon_magazine_capacity() {
        let doc = CharacterDocument::from_text(EQUIPPED).unwrap();
        let mut slots = doc.equipment().unwrap();
        let item = slots[0].item.as_mut().unwrap();
        item.amount = 21;
        item.condition = 101.0;
        let errors = validate_equipment_values(
            &slots,
            &Catalog::load().unwrap(),
            &WeaponStatsBook::load().unwrap(),
        );
        assert!(errors.iter().any(|error| error.contains("maximum 20")));
        assert!(errors.iter().any(|error| error.contains("outside 0–100")));
    }

    #[test]
    fn equipment_amount_and_condition_round_trip() {
        let mut doc = CharacterDocument::from_text(EQUIPPED).unwrap();
        let id = doc.equipment().unwrap()[0]
            .item
            .as_ref()
            .unwrap()
            .resource_id
            .clone();
        doc.set_amount(&id, 25).unwrap();
        doc.set_condition(&id, 77.5).unwrap();
        assert!(doc.is_modified());
        let slots = doc.equipment().unwrap();
        let item = slots[0].item.as_ref().unwrap();
        assert_eq!(item.amount, 25);
        assert_eq!(item.condition, 77.5);
        // Sibling properties in the same sub-resource survive untouched.
        assert!(item.chamber);
        assert_eq!(
            item.nested,
            vec!["res://Items/Attachments/Leopard/Leopard.tres"]
        );
        // An untouched slot stays byte-identical.
        assert!(
            doc.text
                .contains("[sub_resource type=\"Resource\" id=\"Resource_eqsec\"]")
        );
    }

    #[test]
    fn equipment_setters_reject_out_of_range_values() {
        let mut doc = CharacterDocument::from_text(EQUIPPED).unwrap();
        let id = doc.equipment().unwrap()[0]
            .item
            .as_ref()
            .unwrap()
            .resource_id
            .clone();
        assert!(doc.set_amount(&id, -1).is_err());
        assert!(doc.set_condition(&id, 101.0).is_err());
        assert!(doc.set_condition(&id, -0.5).is_err());
        assert!(!doc.is_modified());
    }

    #[test]
    fn adds_missing_item_data_script_without_id_collision() {
        let catalog = Catalog::load().unwrap();
        let bandage = catalog
            .get("res://Items/Medical/Bandage/Bandage.tres")
            .unwrap();
        let mut doc = CharacterDocument::from_text(EMPTY).unwrap();
        let added = doc.add_item(bandage, &catalog).unwrap();
        assert_eq!((added.x, added.y), (0, 0));
        assert_eq!(added.condition, 100.0);
        assert!(
            doc.text
                .contains("path=\"res://Scripts/ItemData.gd\" id=\"3\"")
        );
        assert!(
            doc.text
                .contains("path=\"res://Items/Medical/Bandage/Bandage.tres\" id=\"4\"")
        );
        assert!(doc.validation(&catalog).unwrap().is_valid());
    }

    #[test]
    fn remove_and_edit_round_trip() {
        let catalog = Catalog::load().unwrap();
        let ammo = catalog
            .get("res://Items/Ammo/Ammo_308/Ammo_308.tres")
            .unwrap();
        let mut doc = CharacterDocument::from_text(EMPTY).unwrap();
        let added = doc.add_item(ammo, &catalog).unwrap();
        doc.set_amount(&added.resource_id, 123).unwrap();
        assert_eq!(doc.inventory().unwrap()[0].amount, 123);
        doc.remove_item(&added.resource_id).unwrap();
        assert!(doc.inventory().unwrap().is_empty());
    }

    /// Smoke test against a real save file, only active when `RTV_TEST_SAVE`
    /// points at a readable `Character.tres`. CI skips this test.
    #[test]
    fn live_save_equipment_smoke_test() {
        let Some(path) = std::env::var_os("RTV_TEST_SAVE") else {
            return;
        };
        let text = std::fs::read_to_string(path).expect("read test save");
        let mut doc = CharacterDocument::from_text(&text).expect("load live save");
        let slots = doc.equipment().expect("parse equipment");
        // In-memory only: proves the write path targets the live structure;
        // from_text never touches the disk file.
        if let Some(id) = slots
            .iter()
            .find_map(|slot| slot.item.as_ref().map(|item| item.resource_id.clone()))
        {
            doc.set_amount(&id, 1).expect("set amount in memory");
            doc.set_condition(&id, 99.0)
                .expect("set condition in memory");
            assert!(doc.is_modified());
        }
        assert!(
            !slots.is_empty(),
            "expected the live save to contain equipment slots"
        );
    }

    #[test]
    fn parses_world_mission_clock() {
        let text = r#"[gd_resource type="Resource" script_class="WorldSave" format=3]

[ext_resource type="Script" path="res://Scripts/WorldSave.gd" id="1"]

[resource]
script = ExtResource("1")
difficulty = 1
season = 1
day = 7
time = 245.6749402688046
weather = "Neutral"
weatherTime = 582.4297649499316
shelters = 0
"#;
        let world = parse_world_text(text).expect("should parse World.tres");
        assert_eq!(world.day, 7);
        assert_eq!(world.season, Some(1));
        assert_eq!(world.difficulty, Some(1));
        assert_eq!(world.weather.as_deref(), Some("Neutral"));
        // 245.67 minutes -> 04:05
        assert_eq!(world.clock(), "04:05");
    }

    #[test]
    fn world_clock_wraps_past_midnight() {
        let text =
            "[gd_resource format=3]\n\n[resource]\nday = 12\ntime = 1500.0\nweather = \"Storm\"\n";
        let world = parse_world_text(text).expect("should parse");
        assert_eq!(world.day, 12);
        // 1500 % 1440 = 60 minutes -> 01:00
        assert_eq!(world.clock(), "01:00");

        let before_midnight = WorldState {
            day: 11,
            minutes: -60.0,
            season: None,
            difficulty: None,
            weather: None,
        };
        assert_eq!(before_midnight.clock(), "23:00");
    }

    #[test]
    fn world_parser_only_reads_root_properties() {
        let text = "[gd_resource format=3]\n\n[sub_resource type=\"Resource\" id=\"Fake\"]\nday = 99\ntime = 900.0\n\n[resource]\nday = 3\ntime = 30.0\n";
        let world = parse_world_text(text).expect("should parse root state");
        assert_eq!(world.day, 3);
        assert_eq!(world.clock(), "00:30");
        assert!(
            parse_world_text("[gd_resource format=3]\n\n[resource]\nweather = \"Neutral\"\n")
                .is_none()
        );
    }
}
