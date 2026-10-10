use std::{
    collections::HashMap,
    io,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::{
    catalog::{Catalog, CatalogItem, WeaponStatsBook},
    grid::{CELL_SIZE, COLS, ROWS, ValidationReport},
    telemetry::{
        protocol::{DEFAULT_BIND_ADDRESS, JsonDecoder},
        radar::{RadarMode, contact_cell, project_contact},
        receiver::{ReceiverEvent, TelemetryReceiver},
        state::{ConnectionStatus, SHOT_LIFETIME, TelemetryState, TrackedAi},
    },
    tres::{
        CharacterDocument, EquipmentItem, EquipmentSlot, InventoryItem, SaveBackup, WorldState,
        validate_equipment_values,
    },
};

const RADAR_RANGES: [f64; 4] = [50.0, 100.0, 200.0, 400.0];
const EDITABLE_VITALS: [(&str, &str); 5] = [
    ("health", "Health"),
    ("mental", "Mental"),
    ("energy", "Energy"),
    ("hydration", "Hydration"),
    ("temperature", "Temperature"),
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Panel {
    Character,
    Equipment,
    Inventory,
    Radar,
    Backups,
}

#[derive(Debug)]
enum Mode {
    Normal,
    Add {
        query: String,
        selected: usize,
    },
    EditAmount {
        input: String,
    },
    EditCondition {
        input: String,
    },
    ConfirmDelete,
    ConfirmReload,
    ConfirmRestore {
        path: PathBuf,
        contents: String,
        input: String,
    },
    ConfirmQuit,
    ConfirmAirdrop,
    ConfirmPunisher,
    ConfirmBogeyman,
    SelectVital {
        selected: usize,
    },
    EditVital {
        vital_key: &'static str,
        label: &'static str,
        input: String,
    },
    Help,
}

pub struct App {
    document: CharacterDocument,
    catalog: Catalog,
    weapon_stats: WeaponStatsBook,
    world: Option<WorldState>,
    inventory: Vec<InventoryItem>,
    equipment: Vec<EquipmentSlot>,
    validation: ValidationReport,
    panel: Panel,
    selected: usize,
    equip_selected: usize,
    mode: Mode,
    status: String,
    tab_rects: [Rect; 5],
    list_rect: Rect,
    backups: Vec<SaveBackup>,
    backup_selected: usize,
    backup_list_offset: usize,
    telemetry_receiver: Option<TelemetryReceiver>,
    telemetry_state: TelemetryState,
    telemetry_bind: String,
    pending_summon: Option<(String, Instant)>,
    radar_mode: RadarMode,
    radar_range_index: usize,
    advanced_overlays: bool,
    character_slot_rects: Vec<(usize, Rect)>,
    inventory_list_offset: usize,
    equipment_list_offset: usize,
}

impl App {
    pub fn new(document: CharacterDocument, catalog: Catalog) -> Result<Self> {
        let inventory = document.inventory()?;
        let equipment = document.equipment()?;
        let validation = document.validation(&catalog)?;
        let weapon_stats = WeaponStatsBook::load()?;
        let world = document.world_state();
        // Backup enumeration must never prevent opening a valid save for inspection.
        // Switching to Backups retries and surfaces any directory error.
        let backups = document.backups().unwrap_or_default();
        Ok(Self {
            document,
            catalog,
            weapon_stats,
            world,
            inventory,
            equipment,
            validation,
            panel: Panel::Character,
            selected: 0,
            equip_selected: 0,
            mode: Mode::Normal,
            status: "Loaded save. Press ? for help.".into(),
            tab_rects: [Rect::default(); 5],
            list_rect: Rect::default(),
            backups,
            backup_selected: 0,
            backup_list_offset: 0,
            telemetry_receiver: None,
            telemetry_state: TelemetryState::default(),
            telemetry_bind: DEFAULT_BIND_ADDRESS.to_owned(),
            pending_summon: None,
            radar_mode: RadarMode::Hybrid,
            radar_range_index: 1, // Match the HUD's 100m default; range changes are display-only.
            advanced_overlays: false,
            character_slot_rects: Vec::new(),
            inventory_list_offset: 0,
            equipment_list_offset: 0,
        })
    }

    pub fn enable_telemetry(&mut self, bind_address: &str) {
        self.telemetry_bind = bind_address.to_owned();
        match TelemetryReceiver::bind(bind_address, Arc::new(JsonDecoder)) {
            Ok(receiver) => {
                self.telemetry_receiver = Some(receiver);
                self.status = format!("Listening for live telemetry on UDP {bind_address}.");
            }
            Err(error) => {
                self.telemetry_receiver = None;
                self.telemetry_state.note_socket_error(error.to_string());
                self.status = format!("Telemetry unavailable on {bind_address}: {error}");
            }
        }
    }

    fn poll_telemetry(&mut self) {
        let events = self
            .telemetry_receiver
            .as_ref()
            .map(|receiver| receiver.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for event in events {
            match event {
                ReceiverEvent::Message {
                    received_at,
                    message,
                    ..
                } => self.telemetry_state.apply(message, received_at),
                ReceiverEvent::Malformed { .. } => self.telemetry_state.note_malformed_packet(),
                ReceiverEvent::SocketError(error) => {
                    self.telemetry_state.note_socket_error(error.to_string());
                }
            }
        }
        self.telemetry_state.prune(Instant::now());
    }

    fn poll_summon_result(&mut self) {
        let Some((id, started)) = self.pending_summon.as_ref() else {
            return;
        };
        let Some(directory) = self.document.path().parent() else {
            return;
        };
        let result_path = directory.join("rtv-toolkit-command-result.cfg");
        if let Ok(result) = std::fs::read_to_string(result_path)
            && result.len() <= 4096
        {
            let field = |key: &str| -> Option<&str> {
                result.lines().find_map(|line| {
                    line.trim()
                        .strip_prefix(key)?
                        .strip_prefix("=\"")?
                        .strip_suffix('"')
                })
            };
            if field("id") == Some(id.as_str())
                && let (Some(status @ ("accepted" | "rejected")), Some(message)) =
                    (field("status"), field("message"))
            {
                self.status = format!("Summon {status}: {message}");
                self.pending_summon = None;
                return;
            }
        }
        if started.elapsed() > Duration::from_secs(35) {
            self.status = "Summon result timed out; check the game before trying again.".into();
            self.pending_summon = None;
        }
    }

    pub fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        loop {
            self.poll_telemetry();
            self.poll_summon_result();
            terminal.draw(|frame| self.draw(frame))?;
            if !event::poll(Duration::from_millis(50))? {
                continue;
            }
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if self.handle_key(key)? {
                        return Ok(());
                    }
                }
                Event::Mouse(mouse) => self.handle_mouse(&mouse),
                _ => {}
            }
        }
    }

    fn refresh(&mut self) -> Result<()> {
        self.inventory = self.document.inventory()?;
        self.equipment = self.document.equipment()?;
        self.validation = self.document.validation(&self.catalog)?;
        self.world = self.document.world_state();
        self.selected = self.selected.min(self.inventory.len().saturating_sub(1));
        self.equip_selected = self
            .equip_selected
            .min(self.equipment.len().saturating_sub(1));
        Ok(())
    }

    fn selected_item(&self) -> Option<&InventoryItem> {
        self.inventory.get(self.selected)
    }

    fn selected_equipment_item(&self) -> Option<&EquipmentItem> {
        self.equipment
            .get(self.equip_selected)
            .and_then(|slot| slot.item.as_ref())
    }

    fn selection_len(&self) -> usize {
        match self.panel {
            Panel::Character | Panel::Equipment => self.equipment.len(),
            Panel::Inventory => self.inventory.len(),
            Panel::Backups => self.backups.len(),
            Panel::Radar => 0,
        }
    }

    fn step_selected(&mut self, delta: isize) {
        let len = self.selection_len();
        if len == 0 {
            return;
        }
        let current = match self.panel {
            Panel::Character | Panel::Equipment => self.equip_selected,
            Panel::Inventory => self.selected,
            Panel::Backups => self.backup_selected,
            Panel::Radar => 0,
        } as isize;
        let next = current.saturating_add(delta).clamp(0, len as isize - 1) as usize;
        match self.panel {
            Panel::Character | Panel::Equipment => self.equip_selected = next,
            Panel::Inventory => self.selected = next,
            Panel::Backups => self.backup_selected = next,
            Panel::Radar => {}
        }
    }

    fn select_first(&mut self) {
        match self.panel {
            Panel::Character | Panel::Equipment => self.equip_selected = 0,
            Panel::Inventory => self.selected = 0,
            Panel::Backups => self.backup_selected = 0,
            Panel::Radar => {}
        }
    }

    fn select_last(&mut self) {
        match self.panel {
            Panel::Character | Panel::Equipment => {
                self.equip_selected = self.equipment.len().saturating_sub(1)
            }
            Panel::Inventory => self.selected = self.inventory.len().saturating_sub(1),
            Panel::Backups => self.backup_selected = self.backups.len().saturating_sub(1),
            Panel::Radar => {}
        }
    }

    fn toggle_panel(&mut self) {
        self.switch_panel(match self.panel {
            Panel::Character => Panel::Equipment,
            Panel::Equipment => Panel::Inventory,
            Panel::Inventory => Panel::Radar,
            Panel::Radar => Panel::Backups,
            Panel::Backups => Panel::Character,
        })
    }

    fn previous_panel(&mut self) {
        self.switch_panel(match self.panel {
            Panel::Character => Panel::Backups,
            Panel::Equipment => Panel::Character,
            Panel::Inventory => Panel::Equipment,
            Panel::Radar => Panel::Inventory,
            Panel::Backups => Panel::Radar,
        })
    }

    fn switch_panel(&mut self, panel: Panel) {
        if self.panel == panel {
            return;
        }
        self.panel = panel;
        self.status = match panel {
            Panel::Character => "Character view: u edits offline vitals; s saves safely.".into(),
            Panel::Equipment => "Equipment view; amount and condition edit when applicable.".into(),
            Panel::Inventory => "Inventory view (editable).".into(),
            Panel::Radar => "Live tactical radar; v changes mode and o toggles overlays.".into(),
            Panel::Backups => {
                "Backups for this save only; R restores with confirmation, r refreshes.".into()
            }
        };
        match panel {
            Panel::Equipment => {
                self.equip_selected = self
                    .equip_selected
                    .min(self.equipment.len().saturating_sub(1));
            }
            Panel::Inventory => {
                self.selected = self.selected.min(self.inventory.len().saturating_sub(1));
            }
            Panel::Character => {
                self.equip_selected = self
                    .equip_selected
                    .min(self.equipment.len().saturating_sub(1));
            }
            Panel::Radar => {}
            Panel::Backups => self.refresh_backups(),
        }
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) {
        if !matches!(self.mode, Mode::Normal) {
            return;
        }
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let point = Position::new(mouse.column, mouse.row);
                for (index, rect) in self.tab_rects.iter().enumerate() {
                    if rect.contains(point) {
                        let tabs = [
                            Panel::Character,
                            Panel::Equipment,
                            Panel::Inventory,
                            Panel::Radar,
                            Panel::Backups,
                        ];
                        self.switch_panel(tabs[index]);
                        return;
                    }
                }
                if self.panel == Panel::Character {
                    if let Some((index, _)) = self
                        .character_slot_rects
                        .iter()
                        .find(|(_, rect)| rect.contains(point))
                    {
                        self.equip_selected = *index;
                    }
                    return;
                }
                let list_content = Rect::new(
                    self.list_rect.x.saturating_add(1),
                    self.list_rect.y.saturating_add(1),
                    self.list_rect.width.saturating_sub(2),
                    self.list_rect.height.saturating_sub(2),
                );
                if list_content.contains(point) {
                    let visible_row = mouse.row.saturating_sub(list_content.y) as usize;
                    if self.panel == Panel::Equipment && !self.equipment.is_empty() {
                        let row = self.equipment_list_offset + visible_row;
                        self.equip_selected = row.min(self.equipment.len() - 1);
                    } else if self.panel == Panel::Inventory && !self.inventory.is_empty() {
                        let row = self.inventory_list_offset + visible_row;
                        self.selected = row.min(self.inventory.len() - 1);
                    } else if self.panel == Panel::Backups && !self.backups.is_empty() {
                        let row = self.backup_list_offset + visible_row;
                        self.backup_selected = row.min(self.backups.len() - 1);
                    }
                }
            }
            MouseEventKind::ScrollUp => self.step_selected(-1),
            MouseEventKind::ScrollDown => self.step_selected(1),
            _ => {}
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(true);
        }
        match &mut self.mode {
            Mode::Normal => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    if self.document.is_modified() {
                        self.mode = Mode::ConfirmQuit;
                    } else {
                        return Ok(true);
                    }
                }
                KeyCode::Char('?') | KeyCode::F(1) => self.mode = Mode::Help,
                KeyCode::Char('1') => self.switch_panel(Panel::Character),
                KeyCode::Char('2') => self.switch_panel(Panel::Equipment),
                KeyCode::Char('3') => self.switch_panel(Panel::Inventory),
                KeyCode::Char('4') => self.switch_panel(Panel::Radar),
                KeyCode::Char('5') => self.switch_panel(Panel::Backups),
                KeyCode::Char('i') => self.switch_panel(Panel::Inventory),
                KeyCode::BackTab => self.previous_panel(),
                KeyCode::Tab | KeyCode::Char('e') => self.toggle_panel(),
                KeyCode::Down | KeyCode::Char('j') => self.step_selected(1),
                KeyCode::Up | KeyCode::Char('k') => self.step_selected(-1),
                KeyCode::PageDown => self.step_selected(10),
                KeyCode::PageUp => self.step_selected(-10),
                KeyCode::Home | KeyCode::Char('g') => self.select_first(),
                KeyCode::End | KeyCode::Char('G') => self.select_last(),
                KeyCode::Char('u') if self.panel == Panel::Character => {
                    self.mode = Mode::SelectVital { selected: 0 };
                }
                KeyCode::Char('a') | KeyCode::Char('/') if self.panel == Panel::Inventory => {
                    self.mode = Mode::Add {
                        query: String::new(),
                        selected: 0,
                    }
                }
                KeyCode::Char('d')
                    if self.panel == Panel::Inventory && self.selected_item().is_some() =>
                {
                    self.mode = Mode::ConfirmDelete
                }
                KeyCode::Char('m') => {
                    let amount = match self.panel {
                        Panel::Inventory => self
                            .selected_item()
                            .filter(|item| self.item_has_amount(&item.path))
                            .map(|item| item.amount),
                        Panel::Equipment => self
                            .selected_equipment_item()
                            .filter(|item| self.item_has_amount(&item.path))
                            .map(|item| item.amount),
                        Panel::Character | Panel::Radar | Panel::Backups => None,
                    };
                    if let Some(amount) = amount {
                        self.mode = Mode::EditAmount {
                            input: amount.to_string(),
                        };
                    } else {
                        self.status = "Amount is not applicable to the selected item.".into();
                    }
                }
                KeyCode::Char('c') => {
                    let condition = match self.panel {
                        Panel::Inventory => self
                            .selected_item()
                            .filter(|item| self.item_has_condition(&item.path))
                            .map(|item| item.condition),
                        Panel::Equipment => self
                            .selected_equipment_item()
                            .filter(|item| self.equipment_has_condition(item))
                            .map(|item| item.condition),
                        Panel::Character | Panel::Radar | Panel::Backups => None,
                    };
                    if let Some(condition) = condition {
                        self.mode = Mode::EditCondition {
                            input: format_number(condition),
                        };
                    } else {
                        self.status = "Condition is not applicable to the selected item.".into();
                    }
                }
                KeyCode::Char('s') if self.panel != Panel::Backups => self.save(),
                KeyCode::Char('R') if self.panel == Panel::Backups => self.prepare_restore(),
                KeyCode::Char('r') if self.panel == Panel::Backups => self.refresh_backups(),
                KeyCode::Char('r') => {
                    if self.document.is_modified() {
                        self.mode = Mode::ConfirmReload;
                    } else {
                        self.reload();
                    }
                }
                KeyCode::Char('v') => {
                    if self.panel == Panel::Radar {
                        self.radar_mode = self.radar_mode.cycle();
                        self.status = format!("Radar mode: {}.", self.radar_mode);
                    } else {
                        self.status = if self.validation.is_valid() {
                            format!(
                                "Valid: {} items occupy {} of {} cells.",
                                self.inventory.len(),
                                self.validation.occupied_cells,
                                COLS * ROWS
                            )
                        } else {
                            format!("Invalid: {}", self.validation.errors[0])
                        };
                    }
                }
                KeyCode::Char('A') | KeyCode::Char('p') if self.panel == Panel::Radar => {
                    if self
                        .telemetry_state
                        .can_summon("spawn_airdrop", Instant::now())
                    {
                        self.mode = Mode::ConfirmAirdrop;
                    } else {
                        self.status =
                            "Airdrop unavailable: compatible live summon bridge required.".into();
                    }
                }
                KeyCode::Char('b') if self.panel == Panel::Radar => {
                    if self
                        .telemetry_state
                        .can_summon("spawn_punisher", Instant::now())
                    {
                        self.mode = Mode::ConfirmPunisher;
                    } else {
                        self.status =
                            "Punisher unavailable: compatible live summon bridge required.".into();
                    }
                }
                KeyCode::Char('B') if self.panel == Panel::Radar => {
                    if self
                        .telemetry_state
                        .can_summon("spawn_bogeyman", Instant::now())
                    {
                        self.mode = Mode::ConfirmBogeyman;
                    } else {
                        self.status =
                            "Bogeyman unavailable: compatible live summon bridge required.".into();
                    }
                }
                KeyCode::Char('o') if self.panel == Panel::Radar => {
                    self.advanced_overlays = !self.advanced_overlays;
                    self.status = format!(
                        "Advanced overlays: {} (trails, vision, elevation, loot).",
                        if self.advanced_overlays { "ON" } else { "OFF" }
                    );
                }
                KeyCode::Char('+') | KeyCode::Char('=') if self.panel == Panel::Radar => {
                    self.radar_range_index =
                        (self.radar_range_index + 1).min(RADAR_RANGES.len() - 1);
                    self.status = format!("Radar range: {:.0} m.", self.radar_range());
                }
                KeyCode::Char('-') if self.panel == Panel::Radar => {
                    self.radar_range_index = self.radar_range_index.saturating_sub(1);
                    self.status = format!("Radar range: {:.0} m.", self.radar_range());
                }
                _ => {}
            },
            Mode::Add { query, selected } => {
                let count = self.catalog.filtered_indices(query).len();
                match key.code {
                    KeyCode::Esc => self.mode = Mode::Normal,
                    KeyCode::Down => {
                        if count > 0 {
                            *selected = (*selected + 1).min(count - 1);
                        }
                    }
                    KeyCode::Up => *selected = selected.saturating_sub(1),
                    KeyCode::PageDown => {
                        if count > 0 {
                            *selected = (*selected + 10).min(count - 1);
                        }
                    }
                    KeyCode::PageUp => *selected = selected.saturating_sub(10),
                    KeyCode::Backspace => {
                        query.pop();
                        *selected = 0;
                    }
                    KeyCode::Enter => {
                        let filtered = self.catalog.filtered_indices(query);
                        if let Some(index) = filtered.get(*selected).copied() {
                            let item = self.catalog.items()[index].clone();
                            self.add_item(item);
                        }
                    }
                    KeyCode::Char(character)
                        if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                    {
                        query.push(character);
                        *selected = 0;
                    }
                    _ => {}
                }
            }
            Mode::EditAmount { input } => match key.code {
                KeyCode::Esc => self.mode = Mode::Normal,
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(character) if character.is_ascii_digit() => input.push(character),
                KeyCode::Enter => {
                    let value = input.parse::<i64>();
                    match value {
                        Ok(value) => self.edit_amount(value),
                        Err(_) => self.status = "Amount must be a whole number.".into(),
                    }
                }
                _ => {}
            },
            Mode::EditCondition { input } => match key.code {
                KeyCode::Esc => self.mode = Mode::Normal,
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(character)
                    if character.is_ascii_digit() || (character == '.' && !input.contains('.')) =>
                {
                    input.push(character)
                }
                KeyCode::Enter => {
                    let value = input.parse::<f64>();
                    match value {
                        Ok(value) => self.edit_condition(value),
                        Err(_) => self.status = "Condition must be a number from 0 to 100.".into(),
                    }
                }
                _ => {}
            },
            Mode::SelectVital { selected } => match key.code {
                KeyCode::Esc => self.mode = Mode::Normal,
                KeyCode::Down | KeyCode::Char('j') => {
                    *selected = (*selected + 1).min(EDITABLE_VITALS.len() - 1);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    *selected = selected.saturating_sub(1);
                }
                KeyCode::Enter => {
                    let (vital_key, label) = EDITABLE_VITALS[*selected];
                    let input = self
                        .document
                        .stat(vital_key)
                        .unwrap_or_else(|| "100".to_owned());
                    self.mode = Mode::EditVital {
                        vital_key,
                        label,
                        input,
                    };
                }
                _ => {}
            },
            Mode::EditVital {
                vital_key,
                label,
                input,
            } => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::SelectVital {
                        selected: EDITABLE_VITALS
                            .iter()
                            .position(|(candidate, _)| candidate == vital_key)
                            .unwrap_or(0),
                    }
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(character)
                    if character.is_ascii_digit() || (character == '.' && !input.contains('.')) =>
                {
                    input.push(character);
                }
                KeyCode::Enter => match input.parse::<f64>() {
                    Ok(value) => {
                        let vital_key = *vital_key;
                        let label = *label;
                        self.edit_vital(vital_key, label, value);
                    }
                    Err(_) => self.status = format!("{label} must be a number from 0 to 100."),
                },
                _ => {}
            },
            Mode::ConfirmDelete => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.delete_selected(),
                _ => self.mode = Mode::Normal,
            },
            Mode::ConfirmReload => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.reload(),
                _ => self.mode = Mode::Normal,
            },
            Mode::ConfirmRestore {
                path,
                contents,
                input,
            } => match key.code {
                KeyCode::Esc => self.mode = Mode::Normal,
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) if c.is_ascii_alphabetic() && input.len() < 7 => input.push(c),
                KeyCode::Enter if input == "RESTORE" => {
                    let path = path.clone();
                    let contents = contents.clone();
                    self.restore_backup(&path, &contents);
                }
                KeyCode::Enter => {
                    self.status = "Type RESTORE in uppercase, then Enter to confirm.".into()
                }
                _ => {}
            },
            Mode::ConfirmQuit => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => return Ok(true),
                _ => self.mode = Mode::Normal,
            },
            Mode::ConfirmAirdrop => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.queue_airdrop(),
                _ => self.mode = Mode::Normal,
            },
            Mode::ConfirmPunisher => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.queue_boss("Punisher"),
                _ => self.mode = Mode::Normal,
            },
            Mode::ConfirmBogeyman => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.queue_boss("Bogeyman"),
                _ => self.mode = Mode::Normal,
            },
            Mode::Help => self.mode = Mode::Normal,
        }
        Ok(false)
    }

    fn edit_vital(&mut self, key: &str, label: &str, value: f64) {
        match self.document.set_vital(key, value) {
            Ok(()) => {
                self.mode = Mode::Normal;
                self.status = format!(
                    "{label} set to {}; not saved yet (s saves).",
                    format_number(value)
                );
            }
            Err(error) => {
                self.status = format!("Could not change {label}: {error:#}");
            }
        }
    }

    fn queue_airdrop(&mut self) {
        self.mode = Mode::Normal;
        if !self
            .telemetry_state
            .can_summon("spawn_airdrop", Instant::now())
        {
            self.status = "Airdrop canceled: compatible game bridge is no longer live.".into();
            return;
        }
        match crate::request_airdrop(self.document.path()) {
            Ok(command_id) => {
                self.status =
                    format!("Airdrop request queued ({command_id}); awaiting game result.");
                self.pending_summon = Some((command_id, Instant::now()));
            }
            Err(error) => {
                self.status = format!("Airdrop request failed: {error:#}");
            }
        }
    }

    fn queue_boss(&mut self, name: &str) {
        self.mode = Mode::Normal;
        let action = if name == "Punisher" {
            "spawn_punisher"
        } else {
            "spawn_bogeyman"
        };
        if !self.telemetry_state.can_summon(action, Instant::now()) {
            self.status = format!("{name} canceled: compatible game bridge is no longer live.");
            return;
        }
        match crate::request_runtime_command(self.document.path(), action) {
            Ok(command_id) => {
                self.status =
                    format!("{name} request queued ({command_id}); awaiting game result.");
                self.pending_summon = Some((command_id, Instant::now()));
            }
            Err(error) => {
                self.status = format!("Boss request failed: {error:#}");
            }
        }
    }

    fn add_item(&mut self, item: CatalogItem) {
        match self.document.add_item(&item, &self.catalog) {
            Ok(added) => {
                if let Err(error) = self.refresh() {
                    self.status = format!("Error refreshing inventory: {error:#}");
                    return;
                }
                self.selected = self.inventory.len().saturating_sub(1);
                self.mode = Mode::Normal;
                self.status = format!(
                    "Added {} at ({}, {}){}; not saved yet.",
                    item.name,
                    added.x / CELL_SIZE,
                    added.y / CELL_SIZE,
                    if added.rotated { " rotated" } else { "" }
                );
            }
            Err(error) => self.status = format!("Cannot add {}: {error:#}", item.name),
        }
    }

    fn delete_selected(&mut self) {
        let Some(item) = self.selected_item().cloned() else {
            self.mode = Mode::Normal;
            return;
        };
        let name = self.item_name(&item).to_owned();
        match self.document.remove_item(&item.resource_id) {
            Ok(()) => match self.refresh() {
                Ok(()) => self.status = format!("Removed {name}; not saved yet."),
                Err(error) => self.status = format!("Error refreshing inventory: {error:#}"),
            },
            Err(error) => self.status = format!("Could not remove {name}: {error:#}"),
        }
        self.mode = Mode::Normal;
    }

    fn edit_amount(&mut self, value: i64) {
        if self.panel == Panel::Equipment {
            self.edit_equipment_amount(value);
            return;
        }
        let Some(item) = self.selected_item().cloned() else {
            self.mode = Mode::Normal;
            return;
        };
        if !self.item_has_amount(&item.path) {
            self.status = "Amount is not applicable to the selected item.".into();
            self.mode = Mode::Normal;
            return;
        }
        if let Some(max) = self.max_amount_for(&item.path)
            && value > max
        {
            self.status = format!("Amount {value} exceeds this item's maximum of {max}.");
            return;
        }
        match self.document.set_amount(&item.resource_id, value) {
            Ok(()) => match self.refresh() {
                Ok(()) => {
                    self.mode = Mode::Normal;
                    self.status = format!("Amount changed to {value}; not saved yet.");
                }
                Err(error) => self.status = format!("Error refreshing inventory: {error:#}"),
            },
            Err(error) => self.status = format!("Could not change amount: {error:#}"),
        }
    }

    fn edit_condition(&mut self, value: f64) {
        if self.panel == Panel::Equipment {
            self.edit_equipment_condition(value);
            return;
        }
        let Some(item) = self.selected_item().cloned() else {
            self.mode = Mode::Normal;
            return;
        };
        if !self.item_has_condition(&item.path) {
            self.status = "Condition is not applicable to the selected item.".into();
            self.mode = Mode::Normal;
            return;
        }
        match self.document.set_condition(&item.resource_id, value) {
            Ok(()) => match self.refresh() {
                Ok(()) => {
                    self.mode = Mode::Normal;
                    self.status = format!(
                        "Condition changed to {}; not saved yet.",
                        format_number(value)
                    );
                }
                Err(error) => self.status = format!("Error refreshing inventory: {error:#}"),
            },
            Err(error) => self.status = format!("Could not change condition: {error:#}"),
        }
    }

    fn edit_equipment_amount(&mut self, value: i64) {
        let Some(item) = self.selected_equipment_item().cloned() else {
            self.mode = Mode::Normal;
            return;
        };
        if !self.item_has_amount(&item.path) {
            self.status = "Amount is not applicable to the selected item.".into();
            self.mode = Mode::Normal;
            return;
        }
        if let Some(max) = self.max_amount_for(&item.path)
            && value > max
        {
            self.status = format!("Amount {value} exceeds this item's maximum of {max}.");
            return;
        }
        match self.document.set_amount(&item.resource_id, value) {
            Ok(()) => match self.refresh() {
                Ok(()) => {
                    self.mode = Mode::Normal;
                    self.status = format!(
                        "{} amount set to {value}; not saved yet (s saves).",
                        equipment_name(&item.path)
                    );
                }
                Err(error) => self.status = format!("Error refreshing equipment: {error:#}"),
            },
            Err(error) => self.status = format!("Could not change amount: {error:#}"),
        }
    }

    fn edit_equipment_condition(&mut self, value: f64) {
        let Some(item) = self.selected_equipment_item().cloned() else {
            self.mode = Mode::Normal;
            return;
        };
        if !self.equipment_has_condition(&item) {
            self.status = "Condition is not applicable to the selected item.".into();
            self.mode = Mode::Normal;
            return;
        }
        match self.document.set_condition(&item.resource_id, value) {
            Ok(()) => match self.refresh() {
                Ok(()) => {
                    self.mode = Mode::Normal;
                    self.status = format!(
                        "{} condition set to {}; not saved yet (s saves).",
                        equipment_name(&item.path),
                        format_number(value)
                    );
                }
                Err(error) => self.status = format!("Error refreshing equipment: {error:#}"),
            },
            Err(error) => self.status = format!("Could not change condition: {error:#}"),
        }
    }

    fn save(&mut self) {
        if !self.validation.is_valid() {
            self.status = format!(
                "Refusing to save invalid inventory: {}",
                self.validation.errors[0]
            );
            return;
        }
        if let Some(error) = self.equipment_validation_errors().first() {
            self.status = format!("Refusing to save invalid equipment: {error}");
            return;
        }
        if let Some(error) = self.document.vital_validation_errors().first() {
            self.status = format!("Refusing to save invalid character vital: {error}");
            return;
        }
        if !self.document.is_modified() {
            self.status = "Nothing has changed.".into();
            return;
        }
        match self.document.save() {
            Ok(backup) => self.status = format!("Saved. Backup: {}", backup.display()),
            Err(error) => self.status = format!("Save failed: {error:#}"),
        }
    }

    fn reload(&mut self) {
        match self.document.reload().and_then(|()| self.refresh()) {
            Ok(()) => self.status = "Reloaded from disk; unsaved changes discarded.".into(),
            Err(error) => self.status = format!("Reload failed: {error:#}"),
        }
        self.mode = Mode::Normal;
    }

    fn refresh_backups(&mut self) {
        match self.document.backups() {
            Ok(backups) => {
                self.backups = backups;
                self.backup_selected = self
                    .backup_selected
                    .min(self.backups.len().saturating_sub(1));
                self.status = format!(
                    "{} backups for this character save. No files were changed.",
                    self.backups.len()
                );
            }
            Err(error) => {
                self.backups.clear();
                self.backup_selected = 0;
                self.status = format!("Could not list backups: {error:#}");
            }
        }
    }

    fn prepare_restore(&mut self) {
        if self.document.is_modified() {
            self.status = "Unsaved edits exist; save or reload before restoring a backup.".into();
            return;
        }
        let Some(backup) = self.backups.get(self.backup_selected) else {
            self.status = "No backup selected. Only this save's .rtvbak files are listed.".into();
            return;
        };
        let path = backup.path.clone();
        match CharacterDocument::load(&path).and_then(|doc| {
            doc.inventory()?;
            doc.equipment()?;
            std::fs::read_to_string(&path).map_err(Into::into)
        }) {
            Ok(contents) => {
                self.mode = Mode::ConfirmRestore {
                    path,
                    contents,
                    input: String::new(),
                }
            }
            Err(error) => self.status = format!("Backup cannot be restored: {error:#}"),
        }
    }

    fn restore_backup(&mut self, path: &std::path::Path, contents: &str) {
        match self.document.restore_backup(path, contents) {
            Ok(pre_restore) => {
                self.refresh_backups();
                match self.refresh() {
                    Ok(()) => {
                        self.status = format!(
                            "Restored backup; previous save preserved at {}",
                            pre_restore.display()
                        )
                    }
                    Err(error) => {
                        self.status = format!(
                            "Restore completed but view refresh failed: {error:#}. Previous save: {}",
                            pre_restore.display()
                        )
                    }
                }
            }
            Err(error) => self.status = format!("Restore refused: {error:#}"),
        }
        self.mode = Mode::Normal;
    }

    fn item_name<'a>(&'a self, item: &'a InventoryItem) -> &'a str {
        self.catalog
            .get(&item.path)
            .map(|meta| meta.name.as_str())
            .unwrap_or(&item.path)
    }

    fn item_has_amount(&self, path: &str) -> bool {
        self.catalog
            .get(path)
            .is_some_and(|meta| meta.show_amount || meta.stackable || meta.category == "Weapons")
    }

    fn item_has_condition(&self, path: &str) -> bool {
        self.catalog.get(path).is_some_and(|meta| {
            meta.show_condition || matches!(meta.category.as_str(), "Weapons" | "Armor" | "Helmets")
        })
    }

    fn equipment_has_condition(&self, item: &EquipmentItem) -> bool {
        self.item_has_condition(&item.path)
            || self.catalog.get(&item.path).is_some_and(|meta| {
                meta.category == "Rigs"
                    && item.nested.iter().any(|nested| {
                        self.catalog
                            .get(nested)
                            .is_some_and(|attachment| attachment.category == "Armor")
                    })
            })
    }

    fn max_amount_for(&self, path: &str) -> Option<i64> {
        self.weapon_stats
            .get(path)
            .and_then(|stats| stats.magazine_size)
            .map(|value| value as i64)
            .or_else(|| self.catalog.get(path).and_then(|meta| meta.max_amount))
    }

    fn equipment_validation_errors(&self) -> Vec<String> {
        validate_equipment_values(&self.equipment, &self.catalog, &self.weapon_stats)
    }

    fn draw(&mut self, frame: &mut Frame) {
        frame.render_widget(Block::default().style(canvas_style()), frame.area());

        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(13),
                Constraint::Length(6),
            ])
            .split(frame.area());

        let dirty = if self.document.is_modified() {
            " [MODIFIED]"
        } else {
            ""
        };
        let valid = if self.validation.is_valid() {
            Span::styled("[ VALID ]", status_style(good()))
        } else {
            Span::styled("[ INVALID ]", status_style(danger()))
        };
        let mut header_spans = vec![
            Span::styled(
                " Sobolyatnik-K ",
                Style::default()
                    .bg(accent())
                    .fg(background())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ROAD TO VOSTOK  //  ", label_style()),
            Span::styled(
                format!("{}{dirty}  ", self.document.path().display()),
                Style::default().fg(text_secondary()),
            ),
            valid,
        ];
        if let Some(world) = &self.world {
            header_spans.push(Span::styled(
                format!("  ·  DAY {} {}", world.day, world.clock()),
                Style::default().fg(accent()).add_modifier(Modifier::BOLD),
            ));
        }
        let header = Paragraph::new(Line::from(header_spans))
            .style(panel_style(true))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Double)
                    .border_style(Style::default().fg(accent())),
            );
        frame.render_widget(header, areas[0]);
        self.draw_tab_bar(frame, areas[1]);

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(54), Constraint::Percentage(46)])
            .split(areas[2]);
        match self.panel {
            Panel::Character => self.draw_character(frame, areas[2]),
            Panel::Equipment => {
                self.draw_equipment(frame, columns[0]);
                self.draw_equipment_detail(frame, columns[1], true);
            }
            Panel::Inventory => {
                self.draw_inventory(frame, columns[0]);
                self.draw_grid_and_details(frame, columns[1]);
            }
            Panel::Radar => self.draw_radar(frame, areas[2]),
            Panel::Backups => self.draw_backups(frame, columns[0], columns[1]),
        }

        let command_style = Style::default()
            .fg(accent_bright())
            .add_modifier(Modifier::BOLD);
        let (commands, hint) = match self.panel {
            Panel::Inventory => (
                vec![
                    ("Tab/⇧Tab", "View"),
                    ("↑↓/jk·Pg", "Select"),
                    ("/", "Add"),
                    ("d", "Remove"),
                    ("m", "Ammo"),
                    ("c", "Cond"),
                    ("v", "Check"),
                    ("s", "Save"),
                    ("r", "Reload"),
                    ("?", "Help"),
                    ("q", "Quit"),
                ],
                "INVENTORY — m/c EDIT APPLICABLE VALUES; CHANGES WAIT FOR s",
            ),
            Panel::Equipment => (
                vec![
                    ("Tab/⇧Tab", "View"),
                    ("↑↓/jk·Pg", "Select"),
                    ("m", "Amount"),
                    ("c", "Cond"),
                    ("s", "Save"),
                    ("r", "Reload"),
                    ("?", "Help"),
                    ("q", "Quit"),
                ],
                "EQUIPMENT — m/c EDIT APPLICABLE VALUES; s SAVES SAFELY",
            ),
            Panel::Character => (
                vec![
                    ("Tab/⇧Tab", "View"),
                    ("↑↓/jk", "Select"),
                    ("u", "Vitals"),
                    ("s", "Save"),
                    ("r", "Reload"),
                    ("?", "Help"),
                    ("q", "Quit"),
                ],
                "CHARACTER — u EDITS OFFLINE VITALS; s SAVES SAFELY",
            ),
            Panel::Radar => (
                vec![
                    ("Tab/⇧Tab", "View"),
                    ("v", "Mode"),
                    ("o", "Overlays"),
                    ("p", "Airdrop"),
                    ("b/B", "Boss: Punisher/Bogeyman"),
                    ("+/−", "Range"),
                    ("?", "Help"),
                    ("q", "Quit"),
                ],
                "RADAR — PLAYER-CENTERED · UP IS CURRENT HEADING",
            ),
            Panel::Backups => (
                vec![
                    ("Tab/⇧Tab", "View"),
                    ("↑↓/jk·Pg", "Select"),
                    ("r", "Refresh"),
                    ("R", "Restore"),
                    ("?", "Help"),
                    ("q", "Quit"),
                ],
                "BACKUPS — ONLY MATCHING CHARACTER .rtvbak FILES · NO DELETE",
            ),
        };
        let mut command_spans: Vec<Span<'static>> = Vec::new();
        for (key, action) in commands {
            command_spans.push(Span::styled(key, command_style));
            command_spans.push(Span::raw(format!(" {action}  ")));
        }
        command_spans.push(Span::styled(
            "mouse: click·wheel",
            Style::default().fg(muted()),
        ));
        let footer = Paragraph::new(vec![
            Line::raw(self.status.as_str()),
            Line::from(command_spans),
            Line::styled(hint, Style::default().fg(muted())),
        ])
        .wrap(Wrap { trim: false })
        .style(panel_style(false))
        .block(panel_block("STATUS & COMMANDS"));
        frame.render_widget(footer, areas[3]);

        match &self.mode {
            Mode::Add { query, selected } => self.draw_add_dialog(frame, query, *selected),
            Mode::EditAmount { input } => {
                self.draw_input_dialog(frame, "Edit amount", input, "Enter amount")
            }
            Mode::EditCondition { input } => {
                self.draw_input_dialog(frame, "Edit condition", input, "Enter 0–100")
            }
            Mode::ConfirmDelete => self.draw_confirm(
                frame,
                "Remove selected item?",
                "y/Enter=yes, any other key=no",
            ),
            Mode::ConfirmReload => self.draw_confirm(
                frame,
                "Discard all unsaved changes?",
                "y/Enter=yes, any other key=no",
            ),
            Mode::ConfirmRestore { input, .. } => self.draw_input_dialog(
                frame,
                "Restore selected backup",
                input,
                "Close RTV; type RESTORE to preserve current save and restore",
            ),
            Mode::ConfirmQuit => self.draw_confirm(
                frame,
                "Quit and discard unsaved changes?",
                "y/Enter=yes, any other key=no",
            ),
            Mode::ConfirmAirdrop => self.draw_confirm(
                frame,
                "Trigger one native CASA airdrop event?",
                "y/Enter=yes, any other key=no",
            ),
            Mode::ConfirmPunisher => self.draw_confirm(
                frame,
                "Spawn the native Punisher at a distant game point?",
                "y/Enter=yes, any other key=no",
            ),
            Mode::ConfirmBogeyman => self.draw_confirm(
                frame,
                "Spawn the native Bogeyman at a distant lurk point?",
                "y/Enter=yes, any other key=no",
            ),
            Mode::SelectVital { selected } => self.draw_vital_dialog(frame, *selected),
            Mode::EditVital { label, input, .. } => self.draw_input_dialog(
                frame,
                &format!("Edit {label}"),
                input,
                "Enter value from 0 to 100",
            ),
            Mode::Help => self.draw_help(frame),
            Mode::Normal => {}
        }
    }

    fn draw_tab_bar(&mut self, frame: &mut Frame, area: Rect) {
        let quarters = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(20); 5])
            .split(area);
        self.tab_rects = [
            quarters[0],
            quarters[1],
            quarters[2],
            quarters[3],
            quarters[4],
        ];
        let telemetry_status = self.telemetry_state.connection_status(Instant::now());
        let labels = [
            "[1] CHARACTER".to_string(),
            format!("[2] EQUIPMENT  {} SLOTS", self.equipment.len()),
            format!("[3] INVENTORY  {} ITEMS", self.inventory.len()),
            format!("[4] RADAR  {}", connection_label(telemetry_status)),
            format!("[5] BACKUPS  {}", self.backups.len()),
        ];
        let active_index = match self.panel {
            Panel::Character => 0,
            Panel::Equipment => 1,
            Panel::Inventory => 2,
            Panel::Radar => 3,
            Panel::Backups => 4,
        };
        for (index, rect) in quarters.iter().enumerate() {
            let style = if index == active_index {
                Style::default()
                    .bg(selection_bg())
                    .fg(accent_bright())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(surface()).fg(muted())
            };
            frame.render_widget(
                Paragraph::new(Line::raw(labels[index].as_str()))
                    .style(style)
                    .alignment(Alignment::Center),
                *rect,
            );
        }
    }

    fn radar_range(&self) -> f64 {
        RADAR_RANGES[self.radar_range_index]
    }

    fn draw_radar(&self, frame: &mut Frame, area: Rect) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
            .split(area);
        let now = Instant::now();
        let connection = self.telemetry_state.connection_status(now);
        let map_name = self
            .telemetry_state
            .current_map
            .as_ref()
            .map_or("UNKNOWN", |map| map.name.as_str());
        let radar_block = panel_block(&format!(
            "SOBOLYATNIK-K 1L108K  //  MAP {}  //  {}  //  DETECT {:.0}m  //  {}",
            map_name.to_uppercase(),
            self.radar_mode,
            self.radar_range(),
            connection_label(connection)
        ));
        let radar_inner = radar_block.inner(columns[0]);
        frame.render_widget(radar_block, columns[0]);
        self.draw_radar_scope(frame, radar_inner, now);
        self.draw_telemetry_readings(frame, columns[1], now);
    }

    fn draw_radar_scope(&self, frame: &mut Frame, area: Rect, now: Instant) {
        let width = area.width as usize;
        let height = area.height as usize;
        if width == 0 || height == 0 {
            return;
        }
        let mut cells = vec![vec![(' ', Style::default()); width]; height];
        let center_x = (width / 2) as i32;
        let center_y = (height / 2) as i32;
        let radius_y = center_y.max(1) as f64;
        let radius_x = ((width.saturating_sub(1) / 2) as f64).min(radius_y * 2.0);
        let ring_style = Style::default().fg(border());
        let tolerance = (0.7 / radius_y).max(0.7 / radius_x.max(1.0));
        for (row, cells_row) in cells.iter_mut().enumerate() {
            for (column, cell) in cells_row.iter_mut().enumerate() {
                let dx = (column as f64 - center_x as f64) / radius_x.max(1.0);
                let dy = (row as f64 - center_y as f64) / radius_y;
                let radius = dx.hypot(dy);
                if [1.0 / 3.0, 2.0 / 3.0, 1.0]
                    .iter()
                    .any(|ring| (radius - ring).abs() <= tolerance)
                {
                    *cell = ('·', ring_style);
                }
            }
        }
        for column in 0..width {
            set_radar_cell(
                &mut cells,
                column as i32,
                center_y,
                '─',
                Style::default().fg(muted()),
            );
        }
        for row in 0..height {
            set_radar_cell(
                &mut cells,
                center_x,
                row as i32,
                '│',
                Style::default().fg(muted()),
            );
        }

        if let Some(player) = &self.telemetry_state.player {
            if self.advanced_overlays && self.radar_mode.shows_ai() {
                for entity in self.telemetry_state.ai_entities.values() {
                    for point in &entity.trail {
                        if let Some(contact) = project_contact(
                            player.position,
                            player.heading,
                            point.position,
                            self.radar_range(),
                        ) {
                            let (offset_x, offset_y) =
                                contact_cell(contact, self.radar_range(), radius_x, radius_y);
                            set_radar_cell(
                                &mut cells,
                                center_x + offset_x,
                                center_y + offset_y,
                                '·',
                                Style::default().fg(accent()),
                            );
                        }
                    }
                    if entity.alive
                        && horizontal_distance(player.position, entity.position)
                            <= self.radar_range()
                        && let (Some(heading), Some(range), Some(half_angle)) = (
                            entity.heading,
                            entity.vision_range,
                            entity.vision_half_angle,
                        )
                    {
                        let cone_style = Style::default().fg(if entity.player_visible {
                            danger()
                        } else if entity.faction == "Nomad" {
                            Color::Rgb(66, 121, 150)
                        } else {
                            Color::Rgb(112, 82, 48)
                        });
                        for position in vision_cone_samples(
                            entity.position,
                            heading,
                            range.min(self.radar_range()),
                            half_angle,
                        ) {
                            if let Some(contact) = project_contact(
                                player.position,
                                player.heading,
                                position,
                                self.radar_range(),
                            ) {
                                let (offset_x, offset_y) =
                                    contact_cell(contact, self.radar_range(), radius_x, radius_y);
                                set_radar_cell(
                                    &mut cells,
                                    center_x + offset_x,
                                    center_y + offset_y,
                                    '·',
                                    cone_style,
                                );
                            }
                        }
                    }
                }
            }
            if self.advanced_overlays {
                for loot in self.telemetry_state.loot_containers.values() {
                    if let Some(contact) = project_contact(
                        player.position,
                        player.heading,
                        loot.position,
                        self.radar_range(),
                    ) {
                        let (offset_x, offset_y) =
                            contact_cell(contact, self.radar_range(), radius_x, radius_y);
                        let column = center_x + offset_x;
                        let row = center_y + offset_y;
                        set_radar_cell(
                            &mut cells,
                            column,
                            row,
                            if loot.corpse {
                                '¤'
                            } else if loot.locked {
                                '▣'
                            } else {
                                '□'
                            },
                            Style::default().fg(good()).add_modifier(Modifier::BOLD),
                        );
                        if (offset_x.abs() > 2 || offset_y.abs() > 1)
                            && let Some(indicator) =
                                loot_elevation_indicator(loot.position[1] - player.position[1])
                        {
                            set_radar_cell(
                                &mut cells,
                                column + 1,
                                row,
                                indicator,
                                Style::default().fg(accent_bright()),
                            );
                        }
                    }
                }
            }
            if self.radar_mode.shows_ai() {
                for entity in self.telemetry_state.ai_entities.values() {
                    if let Some(contact) = project_contact(
                        player.position,
                        player.heading,
                        entity.position,
                        self.radar_range(),
                    ) {
                        let (offset_x, offset_y) =
                            contact_cell(contact, self.radar_range(), radius_x, radius_y);
                        let column = center_x + offset_x;
                        let row = center_y + offset_y;
                        set_radar_cell(
                            &mut cells,
                            column,
                            row,
                            if entity.boss && entity.alive {
                                'B'
                            } else if entity.alive {
                                '◆'
                            } else {
                                '×'
                            },
                            Style::default()
                                .fg(ai_contact_color(entity))
                                .add_modifier(Modifier::BOLD),
                        );
                        if self.advanced_overlays
                            && let Some(indicator) =
                                elevation_indicator(entity.position[1] - player.position[1])
                        {
                            set_radar_cell(
                                &mut cells,
                                column + 1,
                                row,
                                indicator,
                                Style::default().fg(if entity.boss {
                                    boss_color()
                                } else {
                                    accent_bright()
                                }),
                            );
                        }
                    }
                }
            }
            if self.radar_mode.shows_shots() {
                for shot in &self.telemetry_state.shot_events {
                    if let Some(contact) = project_contact(
                        player.position,
                        player.heading,
                        shot.position,
                        self.radar_range(),
                    ) {
                        let (offset_x, offset_y) =
                            contact_cell(contact, self.radar_range(), radius_x, radius_y);
                        let age = now.saturating_duration_since(shot.occurred_at);
                        set_radar_cell(
                            &mut cells,
                            center_x + offset_x,
                            center_y + offset_y,
                            '✦',
                            Style::default()
                                .fg(shot_color(age))
                                .add_modifier(Modifier::BOLD),
                        );
                    }
                }
            }
            set_radar_cell(
                &mut cells,
                center_x,
                center_y,
                '▲',
                Style::default()
                    .fg(accent_bright())
                    .add_modifier(Modifier::BOLD),
            );
        } else {
            let message = "WAITING FOR LIVE PLAYER TELEMETRY";
            let start = center_x.saturating_sub(message.chars().count() as i32 / 2);
            for (offset, character) in message.chars().enumerate() {
                set_radar_cell(
                    &mut cells,
                    start + offset as i32,
                    center_y,
                    character,
                    Style::default().fg(muted()),
                );
            }
        }

        let lines = cells
            .into_iter()
            .map(|row| {
                Line::from(
                    row.into_iter()
                        .map(|(character, style)| Span::styled(character.to_string(), style))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(lines).style(panel_style(false)), area);
    }

    fn draw_telemetry_readings(&self, frame: &mut Frame, area: Rect, now: Instant) {
        let connection = self.telemetry_state.connection_status(now);
        let mut lines = vec![
            Line::styled(
                format!("LINK  {}", connection_label(connection)),
                status_style(connection_color(connection)).add_modifier(Modifier::BOLD),
            ),
            Line::styled(
                format!("UDP   {}", self.telemetry_bind),
                Style::default().fg(muted()),
            ),
            Line::styled(
                format!(
                    "PACKETS {}  //  REJECTED {}",
                    self.telemetry_state.packet_count, self.telemetry_state.malformed_packet_count
                ),
                Style::default().fg(text_secondary()),
            ),
            Line::styled(
                format!("DETECT  {:.0}m  (+/-)", self.radar_range()),
                Style::default()
                    .fg(accent_bright())
                    .add_modifier(Modifier::BOLD),
            ),
            Line::styled(
                format!(
                    "OVERLAYS  {}",
                    if self.advanced_overlays { "ON" } else { "OFF" }
                ),
                Style::default().fg(if self.advanced_overlays {
                    good()
                } else {
                    muted()
                }),
            ),
            Line::styled(
                format!(
                    "MAP  {}",
                    self.telemetry_state
                        .current_map
                        .as_ref()
                        .map_or("UNKNOWN", |map| map.name.as_str())
                        .to_uppercase()
                ),
                Style::default().fg(accent_bright()),
            ),
            Line::raw(""),
        ];
        if let Some(error) = &self.telemetry_state.last_socket_error {
            lines.push(Line::styled(
                format!("SOCKET  {error}"),
                Style::default().fg(danger()),
            ));
            lines.push(Line::raw(""));
        }
        if let Some(player) = &self.telemetry_state.player {
            lines.push(Line::styled(
                "PLAYER",
                Style::default().fg(accent()).add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::styled(
                format!("ID {}", player.id),
                Style::default().fg(text_secondary()),
            ));
            lines.push(Line::styled(
                format!(
                    "X {:+.1}  Y {:+.1}  Z {:+.1}",
                    player.position[0], player.position[1], player.position[2]
                ),
                Style::default().fg(text_secondary()),
            ));
            lines.push(Line::styled(
                format!("HEADING {:06.2}°", player.heading),
                Style::default().fg(accent_bright()),
            ));
            lines.push(Line::styled(
                format!(
                    "AGE {:>4.1}s",
                    now.saturating_duration_since(player.last_seen)
                        .as_secs_f64()
                ),
                Style::default().fg(muted()),
            ));
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                if self.radar_mode.shows_ai() {
                    format!(
                        "AI CONTACTS  {}",
                        self.telemetry_state
                            .ai_entities
                            .values()
                            .filter(
                                |entity| horizontal_distance(player.position, entity.position)
                                    <= self.radar_range()
                            )
                            .count()
                    )
                } else {
                    "AI CONTACTS  MASKED".to_owned()
                },
                Style::default().fg(accent()).add_modifier(Modifier::BOLD),
            ));
            if self.radar_mode.shows_ai() {
                let mut contacts = self
                    .telemetry_state
                    .ai_entities
                    .values()
                    .map(|entity| {
                        (
                            horizontal_distance(player.position, entity.position),
                            entity,
                        )
                    })
                    .filter(|(distance, _)| *distance <= self.radar_range())
                    .collect::<Vec<_>>();
                contacts.sort_by(|left, right| left.0.total_cmp(&right.0));
                let contact_limit = if self.advanced_overlays { 7 } else { 12 };
                for (distance, entity) in contacts.into_iter().take(contact_limit) {
                    let elevation = if self.advanced_overlays {
                        elevation_text(entity.position[1] - player.position[1])
                    } else {
                        String::new()
                    };
                    let map_suffix = if entity.boss {
                        format!(
                            " @ {}",
                            if entity.map_name.is_empty() {
                                "UNKNOWN"
                            } else {
                                entity.map_name.as_str()
                            }
                            .to_uppercase()
                        )
                    } else {
                        String::new()
                    };
                    lines.push(Line::styled(
                        format!(
                            "{} #{:04} {:>6.1}m{}{}{}",
                            if entity.boss && entity.alive {
                                "B"
                            } else if entity.alive {
                                "◆"
                            } else {
                                "×"
                            },
                            entity.id % 10_000,
                            distance,
                            if entity.boss {
                                " BOSS"
                            } else if entity.faction == "Nomad" {
                                " NOMAD"
                            } else {
                                ""
                            },
                            elevation,
                            map_suffix
                        ),
                        Style::default().fg(ai_contact_color(entity)),
                    ));
                }
            } else {
                lines.push(Line::styled(
                    "Locations masked in ACOUSTIC mode",
                    Style::default().fg(muted()),
                ));
            }
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                if self.radar_mode.shows_shots() {
                    format!(
                        "RECENT SHOTS  {}",
                        self.telemetry_state
                            .shot_events
                            .iter()
                            .filter(|shot| horizontal_distance(player.position, shot.position)
                                <= self.radar_range())
                            .count()
                    )
                } else {
                    "SHOT DISPLAY  OFF".to_owned()
                },
                Style::default().fg(accent()).add_modifier(Modifier::BOLD),
            ));
            if self.radar_mode.shows_shots() {
                for shot in self
                    .telemetry_state
                    .shot_events
                    .iter()
                    .rev()
                    .filter(|shot| {
                        horizontal_distance(player.position, shot.position) <= self.radar_range()
                    })
                    .take(4)
                {
                    let age = now.saturating_duration_since(shot.occurred_at);
                    lines.push(Line::styled(
                        format!(
                            "✦ #{:04}  {:>4.1}s",
                            shot.shooter_id % 10_000,
                            age.as_secs_f64()
                        ),
                        Style::default().fg(shot_color(age)),
                    ));
                }
            } else {
                lines.push(Line::styled(
                    "Hidden in TRACK mode",
                    Style::default().fg(muted()),
                ));
            }
            if self.advanced_overlays {
                lines.push(Line::raw(""));
                lines.push(Line::styled(
                    format!(
                        "LOOT CONTAINERS  {}",
                        self.telemetry_state
                            .loot_containers
                            .values()
                            .filter(|loot| horizontal_distance(player.position, loot.position)
                                <= self.radar_range())
                            .count()
                    ),
                    Style::default().fg(good()).add_modifier(Modifier::BOLD),
                ));
                let mut loot = self
                    .telemetry_state
                    .loot_containers
                    .values()
                    .map(|container| {
                        (
                            horizontal_distance(player.position, container.position),
                            container,
                        )
                    })
                    .filter(|(distance, _)| *distance <= self.radar_range())
                    .collect::<Vec<_>>();
                loot.sort_by(|left, right| left.0.total_cmp(&right.0));
                for (distance, container) in loot.into_iter().take(6) {
                    let name = if container.name.is_empty() {
                        "Container"
                    } else {
                        container.name.as_str()
                    };
                    lines.push(Line::styled(
                        format!(
                            "{} {:<12.12} {:>5.1}m{}",
                            if container.corpse {
                                "¤"
                            } else if container.locked {
                                "▣"
                            } else {
                                "□"
                            },
                            name,
                            distance,
                            loot_elevation_text(container.position[1] - player.position[1])
                        ),
                        Style::default().fg(good()),
                    ));
                }
            }
        } else {
            lines.push(Line::styled(
                "Start Road to Vostok with the telemetry mod enabled.",
                Style::default().fg(muted()),
            ));
        }
        frame.render_widget(
            Paragraph::new(lines)
                .style(panel_style(false))
                .wrap(Wrap { trim: false })
                .block(panel_block("LIVE TELEMETRY")),
            area,
        );
    }

    fn draw_character(&mut self, frame: &mut Frame, area: Rect) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(area);
        let schematic_width = cols[0].width.saturating_sub(2) as usize;
        self.character_slot_rects = self.character_click_targets(cols[0], schematic_width);
        frame.render_widget(
            Paragraph::new(self.character_schematic_lines(schematic_width))
                .style(panel_style(false))
                .alignment(Alignment::Center)
                .block(panel_block("CHARACTER EQUIPMENT & VITALS")),
            cols[0],
        );
        self.draw_equipment_detail(frame, cols[1], false);
    }

    fn character_click_targets(&self, area: Rect, width: usize) -> Vec<(usize, Rect)> {
        let (left_width, connector_width, center_width, right_width) = schematic_dimensions(width);
        let content_x = area.x.saturating_add(1);
        let content_y = area.y.saturating_add(1);
        let right_x = content_x
            .saturating_add(left_width as u16)
            .saturating_add(connector_width as u16)
            .saturating_add(center_width as u16)
            .saturating_add(connector_width as u16);
        let mut targets = Vec::new();
        let mut add = |requested: &str, row: u16, x: u16, slot_width: usize| {
            if let Some(index) = self
                .equipment
                .iter()
                .position(|slot| normalized_slot(&slot.slot) == normalized_slot(requested))
            {
                targets.push((
                    index,
                    Rect::new(x, content_y.saturating_add(row), slot_width as u16, 1),
                ));
            }
        };
        for (requested, row) in [
            ("Helmet", 4),
            ("NVG", 5),
            ("Primary", 7),
            ("Rig", 8),
            ("Torso", 9),
            ("Knife", 13),
            ("Belt", 14),
            ("Legs", 15),
            ("Feet", 16),
        ] {
            add(requested, row, content_x, left_width);
        }
        for (requested, row) in [
            ("Head", 4),
            ("Hands", 7),
            ("Backpack", 8),
            ("Grenade 1", 13),
            ("Grenade 2", 14),
        ] {
            add(requested, row, right_x, right_width);
        }

        let field_slots = ["Matches", "Light", "Time", "Map", "Player"];
        let base_width = width / field_slots.len();
        let remainder = width % field_slots.len();
        let mut field_x = content_x;
        for (index, requested) in field_slots.into_iter().enumerate() {
            let card_width = base_width + usize::from(index < remainder);
            add(requested, 17, field_x, card_width);
            field_x = field_x.saturating_add(card_width as u16);
        }
        targets
    }

    fn character_schematic_lines(&self, width: usize) -> Vec<Line<'static>> {
        let situation = if let Some(world) = &self.world {
            format!(
                "SITUATION  DAY {} // {}  SEASON {} // DIFF {}  WEATHER {}",
                world.day,
                world.clock(),
                world
                    .season
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "—".to_owned()),
                world
                    .difficulty
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "—".to_owned()),
                world.weather.as_deref().unwrap_or("—").to_uppercase()
            )
        } else {
            "SITUATION  WORLD DATA UNAVAILABLE".to_owned()
        };
        let cat = self.cat_line();
        let cat_text = cat
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();

        let equipped_count = self
            .equipment
            .iter()
            .filter(|slot| slot.item.is_some())
            .count();
        let monitored: Vec<&EquipmentItem> = self
            .equipment
            .iter()
            .filter_map(|slot| slot.item.as_ref())
            .filter(|item| self.equipment_has_condition(item) && item.condition.is_finite())
            .collect();
        let average = if monitored.is_empty() {
            None
        } else {
            Some(monitored.iter().map(|item| item.condition).sum::<f64>() / monitored.len() as f64)
        };
        let readings = format!(
            "[2] EQUIPMENT {}/{} SLOTS AVG {}    [3] INVENTORY {} ITEMS {}/{} CELLS",
            equipped_count,
            self.equipment.len(),
            average.map(percent).unwrap_or_else(|| "—".to_owned()),
            self.inventory.len(),
            self.validation.occupied_cells,
            COLS * ROWS
        );

        let (overall, overall_color) = self.composite_status();
        let threat_count = self.active_threat_count();
        let overall = format!(
            "╰ {:^24} ╯",
            if threat_count == 0 {
                format!("{overall} · NO THREATS")
            } else {
                format!("{overall} · {threat_count} THREATS")
            }
        );

        vec![
            schematic_full_line(&situation, width, label_style()),
            schematic_full_line(&cat_text, width, Style::default().fg(text_secondary())),
            schematic_full_line(&readings, width, Style::default().fg(accent_bright())),
            schematic_full_line("", width, Style::default()),
            self.schematic_row(
                width,
                Some("Helmet"),
                "╭────────── HEAD ──────────╮",
                Some("Head"),
            ),
            self.schematic_vital_row(width, Some("NVG"), "mental", "MENTAL", None),
            self.schematic_row(width, None, "╰─────────────┬────────────╯", None),
            self.schematic_row(
                width,
                Some("Primary"),
                "╭── ARM ─── TORSO ─── ARM ─╮",
                Some("Hands"),
            ),
            self.schematic_vital_row(width, Some("Rig"), "health", "HEALTH", Some("Backpack")),
            self.schematic_vital_row(width, Some("Torso"), "energy", "ENERGY", None),
            self.schematic_vital_row(width, None, "hydration", "HYDRAT", None),
            self.schematic_vital_row(width, None, "temperature", "THERMAL", None),
            self.schematic_row_styled(width, None, &overall, None, status_style(overall_color)),
            self.schematic_row(
                width,
                Some("Knife"),
                "╭────────── BELT ──────────╮",
                Some("Grenade 1"),
            ),
            self.schematic_row(
                width,
                Some("Belt"),
                "╰────────── WAIST ─────────╯",
                Some("Grenade 2"),
            ),
            self.schematic_row(width, Some("Legs"), "╭────────── LEGS ──────────╮", None),
            self.schematic_row(width, Some("Feet"), "╰─── FOOT ──────── FOOT ───╯", None),
            self.schematic_field_row(width),
        ]
    }

    fn composite_status(&self) -> (&'static str, Color) {
        let mut min_vital = 100.0f64;
        for key in ["health", "energy", "hydration", "temperature", "mental"] {
            if let Some(value) = self
                .document
                .stat(key)
                .as_deref()
                .and_then(|raw| raw.parse::<f64>().ok())
            {
                min_vital = min_vital.min(value);
            }
        }
        let flags = self.active_threat_count();
        if min_vital < 20.0 || flags >= 3 {
            ("CRITICAL", danger())
        } else if min_vital < 45.0 || flags > 0 {
            ("DEGRADED", warning())
        } else {
            ("OPERATIONAL", good())
        }
    }

    fn schematic_row(
        &self,
        width: usize,
        left: Option<&str>,
        center: &str,
        right: Option<&str>,
    ) -> Line<'static> {
        self.schematic_row_styled(width, left, center, right, Style::default().fg(border()))
    }

    fn schematic_row_styled(
        &self,
        width: usize,
        left: Option<&str>,
        center: &str,
        right: Option<&str>,
        center_style: Style,
    ) -> Line<'static> {
        let (left_width, connector_width, center_width, right_width) = schematic_dimensions(width);
        let connector = |occupied: bool| {
            if occupied {
                "─".repeat(connector_width)
            } else {
                " ".repeat(connector_width)
            }
        };

        Line::from(vec![
            self.schematic_slot_span(left, left_width, true),
            Span::styled(connector(left.is_some()), Style::default().fg(border())),
            Span::styled(fixed_width(center, center_width), center_style),
            Span::styled(connector(right.is_some()), Style::default().fg(border())),
            self.schematic_slot_span(right, right_width, false),
        ])
    }

    fn schematic_vital_row(
        &self,
        width: usize,
        left: Option<&str>,
        key: &str,
        label: &str,
        right: Option<&str>,
    ) -> Line<'static> {
        let value = self
            .document
            .stat(key)
            .as_deref()
            .and_then(|raw| raw.parse::<f64>().ok())
            .unwrap_or(0.0);
        let center = format!(
            "│ {label:<7} {} {:>4} │",
            compact_meter_pattern(value),
            percent(value)
        );
        self.schematic_row_styled(
            width,
            left,
            &center,
            right,
            status_style(condition_tiers(value).0),
        )
    }

    fn schematic_slot_span(
        &self,
        requested: Option<&str>,
        width: usize,
        align_right: bool,
    ) -> Span<'static> {
        let Some(requested) = requested else {
            return Span::raw(" ".repeat(width));
        };
        let target = normalized_slot(requested);
        let found = self
            .equipment
            .iter()
            .enumerate()
            .find(|(_, slot)| normalized_slot(&slot.slot) == target);
        let Some((index, slot)) = found else {
            return Span::styled(
                fitted_width(
                    &format!("  {} —", requested.to_uppercase()),
                    width,
                    align_right,
                ),
                Style::default().fg(muted()),
            );
        };
        let marker = if index == self.equip_selected {
            '◆'
        } else {
            ' '
        };
        let Some(item) = &slot.item else {
            return Span::styled(
                fitted_width(
                    &format!("{marker} {} —", requested.to_uppercase()),
                    width,
                    align_right,
                ),
                Style::default().fg(muted()),
            );
        };
        let mut name = self
            .catalog
            .get(&item.path)
            .map(|entry| entry.name.clone())
            .unwrap_or_else(|| equipment_name(&item.path));
        let lower_name = name.to_ascii_lowercase();
        let lower_requested = requested.to_ascii_lowercase();
        if lower_name == lower_requested {
            name.clear();
        } else if lower_name.starts_with(&format!("{lower_requested} ")) {
            name = name[requested.len() + 1..].to_owned();
        }
        let reading = if item_is_weapon(self.catalog.get(&item.path), &item.path) {
            let capacity = self
                .max_amount_for(&item.path)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "—".to_owned());
            format!(
                " {}/{}{}",
                item.amount,
                capacity,
                if item.chamber { "+1" } else { "" }
            )
        } else if self.equipment_has_condition(item) {
            let alert = if item.condition <= 25.0 {
                " !"
            } else if item.condition < 70.0 {
                " ▲"
            } else {
                ""
            };
            format!(" {}{alert}", percent(item.condition))
        } else if self.item_has_amount(&item.path) {
            format!(" ×{}", item.amount)
        } else {
            String::new()
        };
        let prefix = format!("{marker} {} ", requested.to_uppercase());
        let name_width = width
            .saturating_sub(prefix.chars().count())
            .saturating_sub(reading.chars().count());
        let fitted_name: String = name.chars().take(name_width).collect();
        let text = format!("{prefix}{fitted_name}{reading}");
        let style = if index == self.equip_selected {
            selection_style()
        } else if self.equipment_has_condition(item) && item.condition <= 25.0 {
            status_style(danger())
        } else if self.equipment_has_condition(item) && item.condition < 70.0 {
            status_style(warning())
        } else {
            Style::default().fg(text_secondary())
        };
        Span::styled(fitted_width(&text, width, align_right), style)
    }

    fn schematic_field_slot_span(&self, requested: &str, width: usize) -> Span<'static> {
        let target = normalized_slot(requested);
        let found = self
            .equipment
            .iter()
            .enumerate()
            .find(|(_, slot)| normalized_slot(&slot.slot) == target);
        let Some((index, slot)) = found else {
            return Span::styled(
                fixed_width(&format!("{} —", requested.to_uppercase()), width),
                Style::default().fg(muted()),
            );
        };
        let marker = if index == self.equip_selected {
            "◆"
        } else {
            ""
        };
        let Some(item) = &slot.item else {
            return Span::styled(
                fixed_width(&format!("{marker}{} —", requested.to_uppercase()), width),
                Style::default().fg(muted()),
            );
        };
        let mut name = self
            .catalog
            .get(&item.path)
            .map(|entry| entry.name.clone())
            .unwrap_or_else(|| equipment_name(&item.path))
            .replace(['(', ')'], "");
        let lower_name = name.to_ascii_lowercase();
        let lower_requested = requested.to_ascii_lowercase();
        if lower_name == lower_requested {
            name.clear();
        } else if lower_name.starts_with(&format!("{lower_requested} ")) {
            name = name[requested.len() + 1..].to_owned();
        }
        let reading = if self.item_has_amount(&item.path) {
            format!("×{}", item.amount)
        } else {
            name.split_whitespace().next().unwrap_or("—").to_owned()
        };
        let text = format!("{marker}{} {reading}", requested.to_uppercase());
        let style = if index == self.equip_selected {
            selection_style()
        } else if self.equipment_has_condition(item) && item.condition <= 25.0 {
            status_style(danger())
        } else if self.equipment_has_condition(item) && item.condition < 70.0 {
            status_style(warning())
        } else {
            Style::default().fg(text_secondary())
        };
        Span::styled(fixed_width(&text, width), style)
    }

    fn schematic_field_row(&self, width: usize) -> Line<'static> {
        let slots = ["Matches", "Light", "Time", "Map", "Player"];
        let base_width = width / slots.len();
        let remainder = width % slots.len();
        let mut spans = Vec::new();
        for (index, slot) in slots.into_iter().enumerate() {
            let card_width = base_width + usize::from(index < remainder);
            if card_width < 2 {
                spans.push(Span::styled(
                    fixed_width("[", card_width),
                    Style::default().fg(border()),
                ));
                continue;
            }
            spans.push(Span::styled("[", Style::default().fg(border())));
            spans.push(self.schematic_field_slot_span(slot, card_width - 2));
            spans.push(Span::styled("]", Style::default().fg(border())));
        }
        Line::from(spans)
    }

    fn active_threat_count(&self) -> usize {
        [
            "starvation",
            "dehydration",
            "bleeding",
            "frostbite",
            "burn",
            "fracture",
            "insanity",
            "rupture",
            "overweight",
        ]
        .iter()
        .filter(|k| self.document.stat(k).as_deref() == Some("true"))
        .count()
    }

    fn cat_line(&self) -> Line<'_> {
        let found = self.document.stat("catFound").as_deref() == Some("true");
        let dead = self.document.stat("catDead").as_deref() == Some("true");
        if dead {
            Line::styled(
                "CAT       IT MADE IT OUT. NO WORDS.",
                Style::default().fg(danger()).add_modifier(Modifier::BOLD),
            )
        } else if found {
            Line::styled(
                "CAT       FOUND — IT LIVES.",
                Style::default().fg(accent()).add_modifier(Modifier::BOLD),
            )
        } else {
            Line::styled(
                "CAT       Somewhere in Vostok.",
                Style::default()
                    .fg(text_secondary())
                    .add_modifier(Modifier::ITALIC),
            )
        }
    }

    /// Compact numeric stat formatting: 50.0 -> "50", 2.5 -> "2.5", None -> "—".
    fn stat_num(value: Option<f64>) -> String {
        match value {
            None => "—".to_string(),
            Some(v) if v.fract() == 0.0 => (v as i64).to_string(),
            Some(v) => format!("{v:.1}"),
        }
    }

    /// Stat block retained as a focused regression-test view of weapon data.
    #[cfg(test)]
    fn weapon_stat_block(
        &self,
        item: &EquipmentItem,
        ws: &crate::catalog::WeaponStats,
    ) -> Vec<Line<'_>> {
        let rarity_suffix = ws
            .rarity
            .map(|r| format!(" · R{}", Self::stat_num(Some(r))))
            .unwrap_or_default();
        let mut lines = vec![Line::from(vec![
            Span::styled(
                format!("   {}", ws.name),
                Style::default()
                    .fg(text_primary())
                    .add_modifier(Modifier::BOLD),
            ),
            match (&ws.weapon_type, &ws.action) {
                (Some(t), Some(a)) => Span::styled(
                    format!("  [{} · {}{}]", t, a, rarity_suffix),
                    Style::default().fg(text_primary()),
                ),
                (Some(t), None) => Span::styled(
                    format!("  [{}{}]", t, rarity_suffix),
                    Style::default().fg(text_primary()),
                ),
                (None, Some(a)) => Span::styled(
                    format!("  [{}{}]", a, rarity_suffix),
                    Style::default().fg(text_primary()),
                ),
                (None, None) => Span::styled(rarity_suffix, Style::default().fg(text_primary())),
            },
        ])];
        lines.push(Line::styled(
            format!(
                "   CAL {}   DMG {}   PEN {}   KICK {}",
                ws.caliber.as_deref().unwrap_or("—"),
                Self::stat_num(ws.damage),
                Self::stat_num(ws.penetration),
                Self::stat_num(ws.kick),
            ),
            Style::default().fg(text_secondary()),
        ));
        lines.push(Line::styled(
            format!(
                "   RATE {}RPM   MAG {}/{}   CHAMBER {}",
                Self::stat_num(ws.rpm),
                item.amount,
                Self::stat_num(ws.magazine_size),
                if item.chamber { "LOADED" } else { "EMPTY" },
            ),
            Style::default().fg(text_secondary()),
        ));
        let mut spans = condition_spans(item.condition);
        spans.insert(
            0,
            Span::styled("   condition ", Style::default().fg(muted())),
        );
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            percent(item.condition),
            Style::default()
                .fg(text_secondary())
                .add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(spans));
        lines
    }

    #[cfg(test)]
    fn armory_lines(&self) -> Vec<Line<'_>> {
        let mut lines = Vec::new();
        let weapon_slots: Vec<&EquipmentSlot> = self
            .equipment
            .iter()
            .filter(|s| {
                s.slot.eq_ignore_ascii_case("Primary") || s.slot.eq_ignore_ascii_case("Secondary")
            })
            .collect();
        for slot in weapon_slots {
            lines.push(Line::styled(
                slot.slot.clone(),
                Style::default().fg(accent()).add_modifier(Modifier::BOLD),
            ));
            match &slot.item {
                Some(item) => {
                    if let Some(ws) = self.weapon_stats.get(&item.path) {
                        lines.extend(self.weapon_stat_block(item, ws));
                    } else {
                        lines.push(Line::styled(
                            format!("   {}", equipment_name(&item.path)),
                            Style::default().fg(text_primary()),
                        ));
                        if self.equipment_has_condition(item) {
                            let mut spans = condition_spans(item.condition);
                            spans.insert(
                                0,
                                Span::styled("   condition ", Style::default().fg(muted())),
                            );
                            spans.push(Span::raw(" "));
                            spans.push(Span::styled(
                                percent(item.condition),
                                Style::default()
                                    .fg(text_secondary())
                                    .add_modifier(Modifier::BOLD),
                            ));
                            lines.push(Line::from(spans));
                        }
                    }
                }
                None => lines.push(Line::styled("   —", Style::default().fg(muted()))),
            }
            lines.push(Line::raw(""));
        }
        lines.push(Line::styled(
            "GEAR",
            Style::default().fg(accent()).add_modifier(Modifier::BOLD),
        ));
        for slot in &self.equipment {
            if !(slot.slot.eq_ignore_ascii_case("Primary")
                || slot.slot.eq_ignore_ascii_case("Secondary"))
            {
                lines.push(self.gear_slot_line(slot));
            }
        }
        lines
    }

    #[cfg(test)]
    fn gear_slot_line(&self, slot: &EquipmentSlot) -> Line<'_> {
        match &slot.item {
            Some(item) => {
                let mut spans = vec![Span::styled(
                    format!("   {}", equipment_name(&item.path)),
                    Style::default().fg(text_primary()),
                )];
                if self.equipment_has_condition(item) {
                    spans.push(Span::raw("  "));
                    spans.extend(condition_spans(item.condition));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        percent(item.condition),
                        Style::default()
                            .fg(text_secondary())
                            .add_modifier(Modifier::BOLD),
                    ));
                } else if self.item_has_amount(&item.path) {
                    spans.push(Span::styled(
                        format!("  ×{}", item.amount),
                        Style::default().fg(text_secondary()),
                    ));
                }
                Line::from(spans)
            }
            None => Line::styled(
                format!("   {}  empty", slot.slot),
                Style::default().fg(muted()),
            ),
        }
    }

    fn draw_backups(&mut self, frame: &mut Frame, list_area: Rect, detail_area: Rect) {
        self.list_rect = list_area;
        let entries = self
            .backups
            .iter()
            .map(|backup| {
                ListItem::new(format!(
                    "{}  {:>8} B",
                    backup_time(backup.modified),
                    backup.bytes
                ))
            })
            .collect::<Vec<_>>();
        let mut state = ListState::default()
            .with_offset(self.backup_list_offset)
            .with_selected((!self.backups.is_empty()).then_some(self.backup_selected));
        frame.render_stateful_widget(
            List::new(entries)
                .style(panel_style(false))
                .highlight_style(selection_style())
                .highlight_symbol("▸ ")
                .block(panel_block(&format!(
                    "SAVE BACKUPS  //  {} FOUND",
                    self.backups.len()
                ))),
            list_area,
            &mut state,
        );
        self.backup_list_offset = state.offset();
        let details = if let Some(backup) = self.backups.get(self.backup_selected) {
            format!(
                "SELECTED BACKUP\n\nFile modified: {}\nSize: {} bytes\n\n{}\n\nR: restore after typing RESTORE.\nThe current save is backed up first.\nNothing is deleted or pruned.",
                backup_time(backup.modified),
                backup.bytes,
                backup.path.display()
            )
        } else {
            format!(
                "No matching backups found for:\n{}\n\nUse s on an editing tab to create a backup when saving changes.\nOnly this save's .rtvbak files appear here.",
                self.document.path().display()
            )
        };
        frame.render_widget(
            Paragraph::new(details)
                .style(panel_style(false))
                .wrap(Wrap { trim: false })
                .block(panel_block("BACKUP DETAILS")),
            detail_area,
        );
    }

    fn draw_inventory(&mut self, frame: &mut Frame, area: Rect) {
        self.list_rect = area;
        let rows = self
            .inventory
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let meta = self.catalog.get(&item.path);
                let name = meta.map(|entry| entry.name.as_str()).unwrap_or("<unknown>");
                let marker = cell_marker(index);
                let conflicting = self.validation.conflicting_ids.contains(&item.resource_id);
                let base = if conflicting {
                    Style::default().fg(danger())
                } else {
                    Style::default().fg(text_secondary())
                };
                // Fixed-width columns: marker(2), name(24), qty(5), cond(31):
                // nothing after the name is variable-width, so every column
                // lines up on every row.
                let amount = if self.item_has_amount(&item.path) {
                    format!("x{:<4}", item.amount)
                } else {
                    "     ".to_owned()
                };
                let mut line = vec![Span::styled(
                    format!("{marker} {name:<24.24}{amount}"),
                    base,
                )];
                let condition_is_meaningful = self.item_has_condition(&item.path);
                line.push(Span::styled("cond ", Style::default().fg(muted())));
                line.extend(if condition_is_meaningful {
                    condition_spans(item.condition)
                } else {
                    neutral_meter_spans(item.condition)
                });
                line.push(Span::styled(
                    format!("{:>5}", percent(item.condition)),
                    Style::default().fg(if condition_is_meaningful {
                        text_secondary()
                    } else {
                        muted()
                    }),
                ));
                ListItem::new(Line::from(line))
            })
            .collect::<Vec<_>>();
        let mut state = ListState::default()
            .with_offset(self.inventory_list_offset)
            .with_selected((!rows.is_empty()).then_some(self.selected));
        let list = List::new(rows)
            .style(panel_style(false))
            .highlight_style(selection_style())
            .highlight_symbol("▸ ")
            .block(panel_block(&format!(
                "INVENTORY  //  {} ITEMS",
                self.inventory.len()
            )));
        frame.render_stateful_widget(list, area, &mut state);
        self.inventory_list_offset = state.offset();
    }

    fn draw_grid_and_details(&self, frame: &mut Frame, area: Rect) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(16), Constraint::Min(6)])
            .split(area);
        let mut owners: HashMap<(usize, usize), usize> = HashMap::new();
        for (index, item) in self.inventory.iter().enumerate() {
            let Some(meta) = self.catalog.get(&item.path) else {
                continue;
            };
            if item.x < 0 || item.y < 0 {
                continue;
            }
            let col = (item.x / CELL_SIZE) as usize;
            let row = (item.y / CELL_SIZE) as usize;
            let (w, h) = meta.size(item.rotated);
            for y in row..row.saturating_add(h).min(ROWS) {
                for x in col..col.saturating_add(w).min(COLS) {
                    owners.insert((x, y), index);
                }
            }
        }
        let mut lines = vec![Line::raw("    0  1  2  3  4  5  6  7")];
        for row in 0..ROWS {
            let mut spans = vec![Span::styled(
                format!("{row:>2} "),
                Style::default().fg(muted()),
            )];
            for col in 0..COLS {
                if let Some(index) = owners.get(&(col, row)).copied() {
                    let item = &self.inventory[index];
                    let mut style = if index == self.selected {
                        Style::default()
                            .bg(selection_bg())
                            .fg(accent_bright())
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(text_secondary())
                    };
                    if self.validation.conflicting_ids.contains(&item.resource_id) {
                        style = Style::default().bg(danger()).fg(text_primary());
                    }
                    spans.push(Span::styled(format!("[{}]", cell_marker(index)), style));
                } else {
                    spans.push(Span::styled("[·]", Style::default().fg(muted())));
                }
            }
            lines.push(Line::from(spans));
        }
        frame.render_widget(
            Paragraph::new(lines)
                .style(panel_style(false))
                .block(panel_block("INVENTORY GRID  //  8×13")),
            rows[0],
        );

        let mut detail = Vec::new();
        if let Some(item) = self.selected_item()
            && let Some(meta) = self.catalog.get(&item.path)
        {
            let (w, h) = meta.size(item.rotated);
            detail.push(Line::from(vec![
                Span::styled(&meta.name, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" [{}]", meta.category)),
            ]));
            let amount = if self.item_has_amount(&item.path) {
                item.amount.to_string()
            } else {
                "n/a".to_owned()
            };
            let condition = if self.item_has_condition(&item.path) {
                percent(item.condition)
            } else {
                format!("{} (stored)", percent(item.condition))
            };
            detail.push(Line::raw(format!(
                "Size {w}×{h}; position {},{}; rotated {}",
                item.x / CELL_SIZE,
                item.y / CELL_SIZE,
                item.rotated,
            )));
            detail.push(Line::raw(format!("Amount {amount}; condition {condition}")));
            detail.push(Line::styled(
                meta.path.as_str(),
                Style::default().fg(muted()),
            ));
        }
        if let Some(error) = self.validation.errors.first() {
            detail.push(Line::styled(error, Style::default().fg(danger())));
        }
        frame.render_widget(
            Paragraph::new(detail)
                .style(panel_style(false))
                .wrap(Wrap { trim: false })
                .block(panel_block("ITEM DETAILS")),
            rows[1],
        );
    }

    fn draw_equipment(&mut self, frame: &mut Frame, area: Rect) {
        self.list_rect = area;
        let rows = self
            .equipment
            .iter()
            .map(|slot| {
                let label = slot_label(&slot.slot);
                let name = slot
                    .item
                    .as_ref()
                    .map(|item| equipment_name(&item.path))
                    .unwrap_or_else(|| "(empty)".to_string());
                let style = match slot.item.as_ref() {
                    None => Style::default().fg(muted()),
                    Some(_) => Style::default().fg(text_secondary()),
                };
                ListItem::new(format!("{label:<14.14} {name}")).style(style)
            })
            .collect::<Vec<_>>();
        let mut state = ListState::default()
            .with_offset(self.equipment_list_offset)
            .with_selected((!rows.is_empty()).then_some(self.equip_selected));
        let list = List::new(rows)
            .style(panel_style(false))
            .highlight_style(selection_style())
            .highlight_symbol("▸ ")
            .block(panel_block(&format!(
                "EQUIPMENT  //  {} SLOTS  //  m AMOUNT · c CONDITION",
                self.equipment.len()
            )));
        frame.render_stateful_widget(list, area, &mut state);
        self.equipment_list_offset = state.offset();
    }

    fn draw_equipment_detail(&self, frame: &mut Frame, area: Rect, editable: bool) {
        let mut lines = Vec::new();
        if let Some(slot) = self.equipment.get(self.equip_selected) {
            lines.push(Line::styled(
                format!("Slot: {}", slot_label(&slot.slot)),
                Style::default().add_modifier(Modifier::BOLD),
            ));
            match &slot.item {
                Some(item) => {
                    let stats = self.weapon_stats.get(&item.path);
                    lines.push(Line::styled(
                        stats
                            .map(|weapon| weapon.name.clone())
                            .unwrap_or_else(|| equipment_name(&item.path)),
                        Style::default().fg(accent()).add_modifier(Modifier::BOLD),
                    ));
                    if let Some(ws) = stats {
                        lines.push(Line::styled(
                            format!(
                                "{}  //  {}  //  CAL {}",
                                ws.weapon_type.as_deref().unwrap_or("WEAPON"),
                                ws.action.as_deref().unwrap_or("—"),
                                ws.caliber.as_deref().unwrap_or("—"),
                            )
                            .to_uppercase(),
                            Style::default().fg(text_primary()),
                        ));
                    }
                    lines.push(Line::styled("", Style::default()));
                    let weapon = item_is_weapon(self.catalog.get(&item.path), &item.path);
                    if self.equipment_has_condition(item) {
                        let mut spans = vec![Span::styled(
                            "CONDITION ",
                            Style::default().fg(accent()).add_modifier(Modifier::BOLD),
                        )];
                        spans.extend(condition_spans(item.condition));
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(
                            percent(item.condition),
                            Style::default().fg(text_secondary()),
                        ));
                        lines.push(Line::from(spans));
                    }
                    if weapon {
                        let capacity = self
                            .max_amount_for(&item.path)
                            .unwrap_or(item.amount.max(0));
                        lines.push(Line::styled(
                            format!("MAGAZINE  {}/{}", item.amount, capacity),
                            Style::default().fg(accent()).add_modifier(Modifier::BOLD),
                        ));
                        lines.extend(ammunition_lines(item.amount, capacity));
                        lines.push(Line::from(vec![
                            Span::styled(
                                if item.chamber { "●" } else { "○" },
                                Style::default().fg(if item.chamber { good() } else { muted() }),
                            ),
                            Span::styled(
                                format!(
                                    " CHAMBER {}",
                                    if item.chamber { "LOADED" } else { "EMPTY" }
                                ),
                                Style::default().fg(text_secondary()),
                            ),
                        ]));
                    } else if self.item_has_amount(&item.path) {
                        lines.push(Line::raw(format!("Amount: {}", item.amount)));
                    }
                    if let Some(ws) = stats {
                        lines.push(Line::styled("", Style::default()));
                        lines.push(Line::styled(
                            "WEAPON READINGS",
                            Style::default().fg(accent()).add_modifier(Modifier::BOLD),
                        ));
                        lines.push(Line::styled(
                            format!(
                                "DAMAGE {}  //  PENETRATION {}",
                                Self::stat_num(ws.damage),
                                Self::stat_num(ws.penetration),
                            ),
                            Style::default().fg(text_secondary()),
                        ));
                        lines.push(Line::styled(
                            format!(
                                "RATE {} RPM  //  KICK {}",
                                Self::stat_num(ws.rpm),
                                Self::stat_num(ws.kick),
                            ),
                            Style::default().fg(text_secondary()),
                        ));
                        lines.push(Line::styled(
                            format!(
                                "WEIGHT {} kg  //  VALUE {}  //  RARITY {}",
                                Self::stat_num(ws.weight),
                                Self::stat_num(ws.value),
                                Self::stat_num(ws.rarity),
                            ),
                            Style::default().fg(text_secondary()),
                        ));
                    }
                    if !item.nested.is_empty() {
                        lines.push(Line::styled("", Style::default()));
                        lines.push(Line::styled(
                            "ATTACHMENTS",
                            Style::default().fg(accent()).add_modifier(Modifier::BOLD),
                        ));
                        for attachment in &item.nested {
                            lines.push(Line::raw(format!(" • {}", equipment_name(attachment))));
                        }
                    }
                    lines.push(Line::styled("", Style::default()));
                    lines.push(Line::styled(
                        "RESOURCE",
                        Style::default().fg(accent()).add_modifier(Modifier::BOLD),
                    ));
                    lines.push(Line::styled(
                        item.path.as_str(),
                        Style::default().fg(muted()),
                    ));
                }
                None => lines.push(Line::styled(
                    "(no item equipped)",
                    Style::default().fg(muted()),
                )),
            }
        } else {
            lines.push(Line::raw("This save has no equipment data."));
            lines.push(Line::raw(
                "Equipment slots appear once the game has created Character.tres.",
            ));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            if editable {
                "m amount · c condition (when applicable) — edits wait for s."
            } else {
                "Read-only here; use [2] Equipment to edit."
            },
            Style::default().fg(muted()),
        ));
        frame.render_widget(
            Paragraph::new(lines)
                .style(panel_style(false))
                .wrap(Wrap { trim: false })
                .block(panel_block(if editable {
                    "EQUIPMENT DETAILS"
                } else {
                    "SELECTED EQUIPMENT"
                })),
            area,
        );
    }

    fn draw_add_dialog(&self, frame: &mut Frame, query: &str, selected: usize) {
        let area = centered_rect(80, 80, frame.area());
        frame.render_widget(Clear, area);
        let dialog =
            dialog_block("ADD ITEM  //  TYPE TO SEARCH · ↑/↓ SELECT · ENTER ADD · ESC CANCEL");
        let inner = dialog.inner(area);
        frame.render_widget(dialog, area);
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1)])
            .split(inner);
        frame.render_widget(
            Paragraph::new(format!(" SEARCH  > {query}█"))
                .style(Style::default().bg(surface_alt()).fg(accent_bright())),
            parts[0],
        );
        let filtered = self.catalog.filtered_indices(query);
        let entries = filtered
            .iter()
            .map(|index| {
                let item = &self.catalog.items()[*index];
                let amount = if item.show_amount || item.stackable {
                    format!(" amount {}", item.default_amount)
                } else {
                    String::new()
                };
                ListItem::new(format!(
                    "{:<25.25} {:<12.12} {}x{}{}",
                    item.name, item.category, item.width, item.height, amount
                ))
            })
            .collect::<Vec<_>>();
        let mut state =
            ListState::default().with_selected((!entries.is_empty()).then_some(selected));
        frame.render_stateful_widget(
            List::new(entries)
                .style(dialog_style())
                .highlight_symbol("▸ ")
                .highlight_style(selection_style()),
            parts[1],
            &mut state,
        );
    }

    fn draw_input_dialog(&self, frame: &mut Frame, title: &str, input: &str, hint: &str) {
        let area = centered_fixed(52, 5, frame.area());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(format!("{hint}: {input}█\nEnter=apply  Esc=cancel"))
                .style(dialog_style())
                .block(dialog_block(&title.to_uppercase())),
            area,
        );
    }

    fn draw_confirm(&self, frame: &mut Frame, question: &str, hint: &str) {
        let area = centered_fixed(60, 5, frame.area());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(format!("{question}\n{hint}"))
                .style(dialog_style())
                .alignment(Alignment::Center)
                .block(alert_block("CONFIRM")),
            area,
        );
    }

    fn draw_vital_dialog(&self, frame: &mut Frame, selected: usize) {
        let area = centered_fixed(54, 11, frame.area());
        frame.render_widget(Clear, area);
        let lines = EDITABLE_VITALS
            .iter()
            .enumerate()
            .map(|(index, (key, label))| {
                let value = self
                    .document
                    .stat(key)
                    .and_then(|raw| raw.parse::<f64>().ok())
                    .map(format_number)
                    .unwrap_or_else(|| "—".to_owned());
                Line::styled(
                    format!(
                        "{} {:<14} {:>7}",
                        if index == selected { "▸" } else { " " },
                        label.to_uppercase(),
                        value
                    ),
                    if index == selected {
                        selection_style()
                    } else {
                        Style::default().fg(text_secondary())
                    },
                )
            })
            .chain(std::iter::once(Line::styled(
                "↑/↓ select · Enter edit · Esc cancel",
                Style::default().fg(muted()),
            )))
            .collect::<Vec<_>>();
        frame.render_widget(
            Paragraph::new(lines)
                .style(dialog_style())
                .block(dialog_block("EDIT OFFLINE VITALS")),
            area,
        );
    }

    fn draw_help(&self, frame: &mut Frame) {
        let area = centered_rect(72, 72, frame.area());
        frame.render_widget(Clear, area);
        let help = [
            "1/2/3/4/5      Character · Equipment · Inventory · Radar · Backups",
            "Tab / Shift-Tab Move one tab right / left",
            "Mouse          Click tabs or list rows; scroll wheel moves selection",
            "Navigation     ↑/↓ or j/k, PgUp/PgDn, Home/End or g/G",
            "u              Edit offline character vitals (Character tab)",
            "/ or a         Search the catalog and add one item (inventory panel)",
            "d              Remove selected inventory item",
            "m              Edit amount/ammunition when applicable",
            "c              Edit condition (0–100) when applicable",
            "v              Radar mode; validate items on other tabs",
            "o              Toggle trails, vision, elevation, and loot",
            "p              Confirm one native CASA airdrop (Radar, live only)",
            "b/B            Confirm Punisher/Bogeyman (Radar, compatible live bridge only)",
            "+ / -          Increase / decrease radar range",
            "s              Save after validation (creates .rtvbak backup)",
            "r              Reload; on Backups tab refresh the list",
            "R              Restore backup on Backups tab; type RESTORE to confirm",
            "F1 or ?        Help        q  Quit",
            "",
            "The equipment panel shows everything the character currently has",
            "equipped, including attachments, magazines, and condition bars.",
            "Only meaningful amount/condition fields can be edited; all changes",
            "remain unsaved until s and use the guarded backup write path.",
            "",
            "Radar is player-centered and rotates so up is the current heading.",
            "TRACK shows AI, ACOUSTIC shows shots, and HYBRID shows both.",
            "Advanced overlays add AI trails/cones, height arrows, and loot.",
            "□ loot  ▣ locked  ¤ corpse  ↑/↓ height  B magenta boss",
            "Bosses always use the magenta B contact, even with overlays off.",
            "Telemetry loss never blocks save editing; stale contacts expire.",
            "",
            "New items are placed by first fit. Normal orientation is tried",
            "first, then rotation. Every occupied cell and the 8×13 bounds",
            "are checked. Saving is refused if validation fails.",
            "",
            "Press any key to close help.",
        ];
        frame.render_widget(
            Paragraph::new(help.join("\n"))
                .style(dialog_style())
                .wrap(Wrap { trim: false })
                .block(dialog_block("HELP")),
            area,
        );
    }
}

/// Display backup timestamps in UTC without depending on the machine's locale.
fn backup_time(when: SystemTime) -> String {
    let seconds = when
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = seconds / 86_400;
    let seconds_in_day = seconds % 86_400;
    // Gregorian civil date from Unix days (400-year era algorithm).
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        seconds_in_day / 3600,
        (seconds_in_day / 60) % 60
    )
}

fn cell_marker(index: usize) -> char {
    const MARKERS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    MARKERS[index % MARKERS.len()] as char
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

fn slot_label(name: &str) -> String {
    name.replace('_', " ")
}

/// Turn an equipment resource path such as
/// `res://res/game/equip/weapon/weapon_m78.xml` into a readable name.
fn equipment_name(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.split('.').next().unwrap_or(file);
    stem.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let first = chars
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default();
            first + chars.as_str()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Formats a condition value (stored on a 0–100 scale) as a percentage string.
fn percent(value: f64) -> String {
    let rounded = value.round();
    let text = if (value - rounded).abs() < 0.05 {
        format!("{rounded:.0}")
    } else {
        format!("{value:.1}")
    };
    format!("{text}%")
}

// Dark tactical palette: blue-black instrument panels, cold cyan telemetry,
// and restrained warning colors. The vocabulary remains Road to Vostok's.
fn background() -> Color {
    Color::Rgb(3, 7, 11)
}

fn surface() -> Color {
    Color::Rgb(7, 16, 23)
}

fn surface_alt() -> Color {
    Color::Rgb(11, 27, 37)
}

fn selection_bg() -> Color {
    Color::Rgb(18, 53, 66)
}

fn border() -> Color {
    Color::Rgb(35, 67, 79)
}

fn accent() -> Color {
    Color::Rgb(83, 181, 196)
}

fn accent_bright() -> Color {
    Color::Rgb(137, 218, 228)
}

fn text_primary() -> Color {
    Color::Rgb(202, 214, 220)
}

fn text_secondary() -> Color {
    Color::Rgb(126, 148, 160)
}

fn muted() -> Color {
    Color::Rgb(72, 94, 105)
}

fn good() -> Color {
    Color::Rgb(103, 163, 126)
}

fn warning() -> Color {
    Color::Rgb(208, 149, 62)
}

fn danger() -> Color {
    Color::Rgb(207, 72, 72)
}

fn connection_label(status: ConnectionStatus) -> &'static str {
    match status {
        ConnectionStatus::Waiting => "WAITING",
        ConnectionStatus::Live => "LIVE",
        ConnectionStatus::Stale => "STALE",
    }
}

fn connection_color(status: ConnectionStatus) -> Color {
    match status {
        ConnectionStatus::Waiting => muted(),
        ConnectionStatus::Live => good(),
        ConnectionStatus::Stale => warning(),
    }
}

// Contact colors match RtVRadarLiteOverlay.gd (Godot RGB floats, rounded to 8-bit).
fn enemy_color() -> Color {
    Color::Rgb(242, 77, 61) // Color(0.95, 0.3, 0.24)
}

fn boss_color() -> Color {
    Color::Rgb(255, 51, 184) // Color(1.0, 0.20, 0.72)
}

fn nomad_color() -> Color {
    Color::Rgb(122, 207, 245) // Color(0.48, 0.81, 0.96)
}

fn ai_contact_color(entity: &TrackedAi) -> Color {
    if entity.boss {
        boss_color()
    } else if entity.faction == "Nomad" && entity.alive {
        nomad_color()
    } else if entity.alive {
        enemy_color()
    } else {
        muted()
    }
}

fn elevation_indicator(delta: f64) -> Option<char> {
    elevation_indicator_with_tolerance(delta, 1.0)
}

fn loot_elevation_indicator(delta: f64) -> Option<char> {
    elevation_indicator_with_tolerance(delta, 2.0)
}

fn elevation_indicator_with_tolerance(delta: f64, tolerance: f64) -> Option<char> {
    if delta >= tolerance {
        Some('↑')
    } else if delta <= -tolerance {
        Some('↓')
    } else {
        None
    }
}

fn elevation_text(delta: f64) -> String {
    match elevation_indicator(delta) {
        Some(indicator) => format!(" {indicator}{:.0}m", delta.abs()),
        None => " =".to_owned(),
    }
}

fn loot_elevation_text(delta: f64) -> String {
    match loot_elevation_indicator(delta) {
        Some(indicator) => format!(" {indicator}{:.0}m", delta.abs()),
        None => " =".to_owned(),
    }
}

fn world_point(origin: [f64; 3], heading: f64, distance: f64) -> [f64; 3] {
    let radians = heading.to_radians();
    [
        origin[0] + radians.sin() * distance,
        origin[1],
        origin[2] - radians.cos() * distance,
    ]
}

fn vision_cone_samples(
    origin: [f64; 3],
    heading: f64,
    range: f64,
    half_angle: f64,
) -> Vec<[f64; 3]> {
    let mut samples = Vec::with_capacity(40);
    for side in [-half_angle, half_angle] {
        for step in 1..=12 {
            samples.push(world_point(
                origin,
                heading + side,
                range * f64::from(step) / 12.0,
            ));
        }
    }
    for step in 0..=16 {
        let angle = heading - half_angle + (2.0 * half_angle * f64::from(step) / 16.0);
        samples.push(world_point(origin, angle, range));
    }
    samples
}

fn horizontal_distance(player: [f64; 3], target: [f64; 3]) -> f64 {
    (target[0] - player[0]).hypot(target[2] - player[2])
}

fn shot_color(age: Duration) -> Color {
    // Terminals have no per-cell alpha. Fade orange toward the scope's dark
    // background at the same five-second lifetime used by the shot history.
    let opacity = (1.0 - age.as_secs_f64() / SHOT_LIFETIME.as_secs_f64()).clamp(0.0, 1.0);
    let blend = |bright: f64, dark: f64| (dark + (bright - dark) * opacity).round() as u8;
    Color::Rgb(blend(255.0, 3.0), blend(123.0, 7.0), blend(63.0, 11.0))
}

fn set_radar_cell(
    cells: &mut [Vec<(char, Style)>],
    column: i32,
    row: i32,
    character: char,
    style: Style,
) {
    if row < 0 || column < 0 {
        return;
    }
    if let Some(cell) = cells
        .get_mut(row as usize)
        .and_then(|line| line.get_mut(column as usize))
    {
        *cell = (character, style);
    }
}

fn canvas_style() -> Style {
    Style::default().bg(background()).fg(text_primary())
}

fn panel_style(emphasized: bool) -> Style {
    Style::default()
        .bg(if emphasized { surface_alt() } else { surface() })
        .fg(text_primary())
}

fn dialog_style() -> Style {
    Style::default().bg(surface_alt()).fg(text_primary())
}

fn label_style() -> Style {
    Style::default()
        .fg(accent_bright())
        .add_modifier(Modifier::BOLD)
}

fn status_style(color: Color) -> Style {
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn selection_style() -> Style {
    Style::default()
        .bg(selection_bg())
        .fg(accent_bright())
        .add_modifier(Modifier::BOLD)
}

fn panel_block(title: &str) -> Block<'static> {
    Block::default()
        .style(panel_style(false))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border()))
        .title(Line::styled(format!(" {title} "), label_style()))
}

fn dialog_block(title: &str) -> Block<'static> {
    Block::default()
        .style(dialog_style())
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(accent()))
        .title(Line::styled(format!(" {title} "), label_style()))
}

fn alert_block(title: &str) -> Block<'static> {
    Block::default()
        .style(dialog_style())
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(danger()))
        .title(Line::styled(format!(" {title} "), status_style(danger())))
}

fn schematic_dimensions(width: usize) -> (usize, usize, usize, usize) {
    const CENTER_WIDTH: usize = 28;
    const CONNECTOR_WIDTH: usize = 2;
    let center_width = CENTER_WIDTH.min(width);
    let available = width.saturating_sub(center_width);
    let connectors_width = (CONNECTOR_WIDTH * 2).min(available);
    let side_width = available.saturating_sub(connectors_width);
    let left_width = side_width / 2;
    let right_width = side_width.saturating_sub(left_width);
    (left_width, connectors_width / 2, center_width, right_width)
}

fn fixed_width(text: &str, width: usize) -> String {
    fitted_width(text, width, false)
}

fn fitted_width(text: &str, width: usize, align_right: bool) -> String {
    let fitted: String = text.chars().take(width).collect();
    let padding = width.saturating_sub(fitted.chars().count());
    if align_right {
        format!("{}{fitted}", " ".repeat(padding))
    } else {
        format!("{fitted}{}", " ".repeat(padding))
    }
}

fn centered_width(text: &str, width: usize) -> String {
    let fitted: String = text.chars().take(width).collect();
    let padding = width.saturating_sub(fitted.chars().count());
    let left = padding / 2;
    format!("{}{fitted}{}", " ".repeat(left), " ".repeat(padding - left))
}

fn normalized_slot(slot: &str) -> String {
    slot.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn schematic_full_line(text: &str, width: usize, style: Style) -> Line<'static> {
    Line::from(Span::styled(centered_width(text, width), style))
}

fn compact_meter_pattern(value: f64) -> String {
    let ticks = value.clamp(0.0, 100.0) * 0.05;
    let mut out = String::with_capacity(11);
    out.push('[');
    for index in 0..5 {
        if index > 0 {
            out.push(' ');
        }
        let at = index as f64;
        out.push(if ticks >= at + 1.0 {
            '▮'
        } else if ticks >= at + 0.5 {
            '▌'
        } else {
            '▯'
        });
    }
    out.push(']');
    out
}

/// Ten separated instrument indicators for a value on the game's 0–100
/// scale. Spacing and outlined empty cells keep a full meter from turning
/// into one solid block of color.
fn condition_pattern(value: f64) -> String {
    let ticks = value.clamp(0.0, 100.0) * 0.1;
    let mut out = String::with_capacity(21);
    out.push('[');
    for i in 0..10 {
        if i > 0 {
            out.push(' ');
        }
        let at = i as f64;
        if ticks >= at + 1.0 {
            out.push('▮');
        } else if ticks >= at + 0.5 {
            out.push('▌');
        } else {
            out.push('▯');
        }
    }
    out.push(']');
    out
}

/// Severity palette for a 0–100 value: (filled, half, empty).
fn condition_tiers(value: f64) -> (Color, Color, Color) {
    let empty = Color::Rgb(31, 48, 57);
    if value >= 70.0 {
        (good(), Color::Rgb(70, 116, 88), empty)
    } else if value >= 30.0 {
        (warning(), Color::Rgb(151, 105, 43), empty)
    } else {
        (danger(), Color::Rgb(145, 51, 51), empty)
    }
}

fn meter_spans(value: f64, filled: Color, partial: Color) -> Vec<Span<'static>> {
    let empty = Color::Rgb(31, 48, 57);
    condition_pattern(value)
        .chars()
        .map(|c| match c {
            '▮' => Span::styled(c.to_string(), Style::default().fg(filled)),
            '▌' => Span::styled(c.to_string(), Style::default().fg(partial)),
            '▯' => Span::styled(c.to_string(), Style::default().fg(empty)),
            '[' | ']' => Span::styled(c.to_string(), Style::default().fg(border())),
            ' ' => Span::raw(" "),
            _ => Span::styled(c.to_string(), Style::default().fg(empty)),
        })
        .collect()
}

/// Render a condition or vital meter using semantic severity colors.
fn condition_spans(value: f64) -> Vec<Span<'static>> {
    let (filled, partial, _) = condition_tiers(value);
    meter_spans(value, filled, partial)
}

/// Render informational values, such as inventory occupancy or stored but
/// non-gameplay condition, without implying that a low value is dangerous.
fn neutral_meter_spans(value: f64) -> Vec<Span<'static>> {
    meter_spans(value, accent(), Color::Rgb(52, 123, 136))
}

fn ammunition_lines(amount: i64, capacity: i64) -> Vec<Line<'static>> {
    let capacity = capacity.clamp(0, 40) as usize;
    let loaded = amount.clamp(0, capacity as i64) as usize;
    (0..capacity)
        .collect::<Vec<_>>()
        .chunks(10)
        .map(|chunk| {
            let mut spans = Vec::with_capacity(chunk.len() * 2);
            for index in chunk {
                spans.push(Span::styled(
                    if *index < loaded { "●" } else { "○" },
                    Style::default().fg(if *index < loaded { accent() } else { muted() }),
                ));
                spans.push(Span::raw(" "));
            }
            Line::from(spans)
        })
        .collect()
}

/// True when an equipped item is a gun. Chamber and magazine details are
/// only meaningful for weapons, so the detail pane uses this to decide
/// whether to show them. The catalog category wins when the item is known
/// (`data/items.json`); otherwise the resource path is a reasonable proxy
/// (e.g. `res://res/game/equip/weapon_*`).
fn item_is_weapon(catalog_entry: Option<&CatalogItem>, path: &str) -> bool {
    match catalog_entry {
        Some(entry) => entry.category.eq_ignore_ascii_case("weapons"),
        None => path.to_ascii_lowercase().contains("weapon"),
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn centered_fixed(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

#[cfg(test)]
mod ui_tests {
    use super::{
        App, Mode, Panel, RadarMode, background, backup_time, boss_color, condition_pattern,
        condition_tiers, danger, elevation_indicator, enemy_color, good, item_is_weapon,
        loot_elevation_indicator, nomad_color, percent, shot_color, surface, vision_cone_samples,
        warning,
    };
    use crate::catalog::Catalog;
    use crate::telemetry::protocol::{
        AiSnapshot, Gunshot, LootSnapshot, MapSnapshot, PlayerSnapshot, Snapshot, TelemetryMessage,
    };
    use crate::tres::CharacterDocument;
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};
    use std::{
        fs,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    const TEST_SCHEMATIC_WIDTH: usize = 82;

    const TEST_CHARACTER: &str = r#"[gd_resource type="Resource" script_class="CharacterSave" format=3]

[ext_resource type="Script" path="res://Scripts/SlotData.gd" id="1"]
[ext_resource type="Script" path="res://Scripts/ItemData.gd" id="2"]
[ext_resource type="Resource" path="res://Items/Weapons/M78/M78.tres" id="3"]
[ext_resource type="Script" path="res://Scripts/CharacterSave.gd" id="5"]

[sub_resource type="Resource" id="Resource_eqpri"]
script = ExtResource("1")
itemData = ExtResource("3")
condition = 93
amount = 18
chamber = true
slot = "Primary"

[sub_resource type="Resource" id="Resource_eqsec"]
script = ExtResource("1")
condition = 100
amount = 0
chamber = false
slot = "Secondary"

[resource]
script = ExtResource("5")
health = 100.0
energy = 100.0
hydration = 100.0
temperature = 100.0
mental = 100.0
inventory = Array[ExtResource("2")]([])
equipment = Array[ExtResource("2")]([SubResource("Resource_eqpri"), SubResource("Resource_eqsec")])
catalog = Array[ExtResource("2")]([])
"#;

    fn test_app() -> App {
        App::new(
            CharacterDocument::from_text(TEST_CHARACTER).unwrap(),
            Catalog::load().unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn summon_result_requires_matching_id_and_reports_rejection() {
        let directory = std::env::temp_dir().join(format!(
            "rtv-summon-result-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let save = directory.join("Character.tres");
        fs::write(&save, TEST_CHARACTER).unwrap();
        let mut app = App::new(
            CharacterDocument::load(&save).unwrap(),
            Catalog::load().unwrap(),
        )
        .unwrap();
        app.pending_summon = Some(("expected".into(), Instant::now()));
        let result = directory.join("rtv-toolkit-command-result.cfg");
        fs::write(
            &result,
            "[result]\nid=\"other\"\nstatus=\"accepted\"\nmessage=\"old\"\n",
        )
        .unwrap();
        app.poll_summon_result();
        assert!(app.pending_summon.is_some());
        fs::write(
            &result,
            "[result]\nid=\"expected\"\nstatus=\"rejected\"\nmessage=\"NO SAFE BOGEYMAN WAYPOINT\"\n",
        )
        .unwrap();
        app.poll_summon_result();
        assert!(app.pending_summon.is_none());
        assert_eq!(app.status, "Summon rejected: NO SAFE BOGEYMAN WAYPOINT");
        fs::remove_dir_all(directory).unwrap();
    }

    fn filled_count(bar: &str) -> usize {
        bar.chars().filter(|c| *c == '▮').count()
    }

    #[test]
    fn header_and_footer_content_are_visible() {
        let mut app = test_app();
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("Sobolyatnik-K"));
        assert!(rendered.contains("Loaded save. Press ? for help."));
        assert!(rendered.contains("CHARACTER — u EDITS OFFLINE VITALS"));
        assert_eq!(
            terminal.backend().buffer().cell((100, 1)).unwrap().bg,
            Color::Rgb(11, 27, 37)
        );
        assert_eq!(
            terminal.backend().buffer().cell((1, 5)).unwrap().bg,
            surface()
        );
    }

    #[test]
    fn character_vital_editor_updates_only_the_offline_document() {
        let mut app = test_app();
        app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE))
            .unwrap();
        assert!(matches!(app.mode, Mode::SelectVital { selected: 0 }));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .unwrap();
        let Mode::EditVital { input, .. } = &mut app.mode else {
            panic!("expected vital input mode");
        };
        input.clear();
        input.push_str("72.5");
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .unwrap();
        assert!(matches!(app.mode, Mode::Normal));
        assert_eq!(app.document.stat("health").as_deref(), Some("72.5"));
        assert!(app.document.is_modified());
        assert!(app.status.contains("not saved yet"));
    }

    #[test]
    fn radar_renders_live_heading_relative_contacts() {
        let mut app = test_app();
        app.panel = Panel::Radar;
        app.telemetry_state.apply(
            TelemetryMessage::Snapshot(Snapshot {
                version: 1,
                timestamp_ms: 1,
                summon_actions: vec![
                    "spawn_airdrop".into(),
                    "spawn_punisher".into(),
                    "spawn_bogeyman".into(),
                ],
                map: MapSnapshot {
                    id: "res://Scenes/Village.tscn".to_owned(),
                    name: "Village".to_owned(),
                },
                player: PlayerSnapshot {
                    id: 10,
                    position: [0.0, 0.0, 0.0],
                    heading: 90.0,
                },
                ai: vec![
                    AiSnapshot {
                        id: 20,
                        position: [25.0, 0.0, 0.0],
                        alive: true,
                        heading: Some(90.0),
                        vision_range: Some(100.0),
                        vision_half_angle: Some(60.0),
                        player_visible: false,
                        boss: false,
                        faction: "Bandit".to_owned(),
                        friendly: false,
                    },
                    AiSnapshot {
                        id: 21,
                        position: [20.0, 0.0, -10.0],
                        alive: true,
                        heading: Some(0.0),
                        vision_range: Some(150.0),
                        vision_half_angle: Some(75.0),
                        player_visible: false,
                        boss: false,
                        faction: "Nomad".to_owned(),
                        friendly: true,
                    },
                    AiSnapshot {
                        id: 22,
                        position: [35.0, 0.0, -30.0],
                        alive: true,
                        heading: Some(0.0),
                        vision_range: Some(150.0),
                        vision_half_angle: Some(75.0),
                        player_visible: true,
                        boss: false,
                        faction: "Nomad".to_owned(),
                        friendly: false,
                    },
                ],
                loot: Vec::new(),
            }),
            Instant::now(),
        );
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("SOBOLYATNIK-K 1L108K"));
        assert!(rendered.contains("MAP VILLAGE"));
        assert!(rendered.contains("HYBRID"));
        assert!(rendered.contains("HEADING 090.00°"));
        assert!(rendered.contains("◆ #0020"));
        assert!(rendered.contains("◆ #0021"));
        assert!(rendered.contains("◆ #0022"));
        assert!(rendered.contains("NOMAD"));
        // Enemy red and both friendly and hostile Nomad blue match the game HUD.
        // LOS/cone colors still communicate a live threat independently.
        let buffer = terminal.backend().buffer();
        for (label, expected_color) in [
            ("#0020", enemy_color()),
            ("#0021", nomad_color()),
            ("#0022", nomad_color()),
        ] {
            let marker_color = buffer.content().chunks(120).find_map(|row| {
                let index = row.windows(5).position(|cells| {
                    cells.iter().map(|cell| cell.symbol()).collect::<String>() == label
                })?;
                row.get(index.checked_sub(2)?).map(|cell| cell.fg)
            });
            assert_eq!(marker_color, Some(expected_color), "{label}");
        }
        assert!(rendered.contains("Boss"));
        app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE))
            .unwrap();
        assert!(matches!(app.mode, Mode::ConfirmPunisher));
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE))
            .unwrap();
        assert!(matches!(app.mode, Mode::Normal));
    }

    #[test]
    fn ai_contact_palette_matches_in_game_mod_source() {
        let overlay = include_str!("../godot-mod/rtv-radar-lite/RtVRadarLiteOverlay.gd");
        assert!(overlay.contains("const NOMAD := Color(0.48, 0.81, 0.96, 1.0)"));
        assert!(overlay.contains("return Color(0.95, 0.3, 0.24)"));
        assert!(overlay.contains("return Color(1.0, 0.20, 0.72)"));
        assert_eq!(nomad_color(), Color::Rgb(122, 207, 245));
        assert_eq!(enemy_color(), Color::Rgb(242, 77, 61));
        assert_eq!(boss_color(), Color::Rgb(255, 51, 184));
    }

    #[test]
    fn radar_mode_and_range_keys_cycle_predictably() {
        let mut app = test_app();
        app.panel = Panel::Radar;
        assert_eq!(app.radar_mode, RadarMode::Hybrid);
        app.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE))
            .unwrap();
        assert_eq!(app.radar_mode, RadarMode::Track);
        app.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE))
            .unwrap();
        assert_eq!(app.radar_mode, RadarMode::Acoustic);
        let range = app.radar_range();
        app.handle_key(KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE))
            .unwrap();
        assert!(app.radar_range() < range);
        assert!(!app.advanced_overlays);
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE))
            .unwrap();
        assert!(app.advanced_overlays);
    }

    #[test]
    fn selected_detection_range_filters_scope_and_telemetry_lists() {
        let mut app = test_app();
        app.panel = Panel::Radar;
        app.advanced_overlays = true;
        assert_eq!(app.radar_range(), 100.0);
        let map_id = "res://Scenes/Outpost.tscn".to_owned();
        let now = Instant::now();
        app.telemetry_state.apply(
            TelemetryMessage::Snapshot(Snapshot {
                version: 1,
                timestamp_ms: 1,
                summon_actions: vec![
                    "spawn_airdrop".into(),
                    "spawn_punisher".into(),
                    "spawn_bogeyman".into(),
                ],
                map: MapSnapshot {
                    id: map_id.clone(),
                    name: "Outpost".to_owned(),
                },
                player: PlayerSnapshot {
                    id: 10,
                    position: [0.0, 0.0, 0.0],
                    heading: 0.0,
                },
                ai: [50.0, 150.0]
                    .into_iter()
                    .enumerate()
                    .map(|(i, x)| AiSnapshot {
                        id: (i + 1) as u64,
                        position: [x, 0.0, 0.0],
                        alive: true,
                        heading: None,
                        vision_range: None,
                        vision_half_angle: None,
                        player_visible: false,
                        boss: false,
                        faction: "Enemy".to_owned(),
                        friendly: false,
                    })
                    .collect(),
                loot: [50.0, 150.0]
                    .into_iter()
                    .enumerate()
                    .map(|(i, x)| LootSnapshot {
                        id: (i + 1) as u64,
                        position: [x, 0.0, 0.0],
                        name: format!("Cache {}", i + 1),
                        locked: false,
                        corpse: false,
                    })
                    .collect(),
            }),
            now,
        );
        for (i, x) in [50.0, 150.0].into_iter().enumerate() {
            app.telemetry_state.apply(
                TelemetryMessage::Gunshot(Gunshot {
                    version: 1,
                    timestamp_ms: 2,
                    shooter_id: (i + 1) as u64,
                    position: [x, 0.0, 0.0],
                    map_id: map_id.clone(),
                }),
                now,
            );
        }
        assert_eq!(app.telemetry_state.shot_events.len(), 2); // Collection unchanged.
        let mut terminal = Terminal::new(TestBackend::new(160, 44)).unwrap();
        let render = |terminal: &mut Terminal<TestBackend>, app: &mut App| {
            terminal.draw(|frame| app.draw(frame)).unwrap();
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
        };
        let near = render(&mut terminal, &mut app);
        assert!(near.contains("DETECT  100m"));
        assert!(near.contains("AI CONTACTS  1"));
        assert!(near.contains("RECENT SHOTS  1"));
        assert!(near.contains("LOOT CONTAINERS  1"));
        assert!(near.contains("Cache 1"));
        assert!(!near.contains("Cache 2"));
        app.handle_key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE))
            .unwrap();
        let wide = render(&mut terminal, &mut app);
        assert!(wide.contains("DETECT  200m"));
        assert!(wide.contains("AI CONTACTS  2"));
        assert!(wide.contains("RECENT SHOTS  2"));
        assert!(wide.contains("LOOT CONTAINERS  2"));
        assert!(wide.contains("Cache 2"));
        app.handle_key(KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE))
            .unwrap();
        assert_eq!(app.radar_range(), 100.0);
    }

    #[test]
    fn advanced_radar_renders_boss_elevation_and_loot() {
        let mut app = test_app();
        app.panel = Panel::Radar;
        app.advanced_overlays = true;
        app.telemetry_state.apply(
            TelemetryMessage::Snapshot(Snapshot {
                version: 1,
                timestamp_ms: 1,
                summon_actions: vec![
                    "spawn_airdrop".into(),
                    "spawn_punisher".into(),
                    "spawn_bogeyman".into(),
                ],
                map: MapSnapshot {
                    id: "res://Scenes/Outpost.tscn".to_owned(),
                    name: "Outpost".to_owned(),
                },
                player: PlayerSnapshot {
                    id: 10,
                    position: [0.0, 0.0, 0.0],
                    heading: 0.0,
                },
                ai: vec![AiSnapshot {
                    id: 99,
                    position: [0.0, 5.0, -20.0],
                    alive: true,
                    heading: Some(180.0),
                    vision_range: Some(50.0),
                    vision_half_angle: Some(60.0),
                    player_visible: true,
                    boss: true,
                    faction: "Boss".to_owned(),
                    friendly: false,
                }],
                loot: vec![LootSnapshot {
                    id: 80,
                    position: [10.0, -3.0, -10.0],
                    name: "Military Crate".to_owned(),
                    locked: false,
                    corpse: false,
                }],
            }),
            Instant::now(),
        );
        let backend = TestBackend::new(160, 44);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("OVERLAYS  ON"));
        assert!(rendered.contains("MAP OUTPOST"));
        assert!(rendered.contains("BOSS ↑5m @ OUTPOST"));
        let boss_label_color = terminal
            .backend()
            .buffer()
            .content()
            .chunks(160)
            .find_map(|row| {
                let index = row.windows(5).position(|cells| {
                    cells.iter().map(|cell| cell.symbol()).collect::<String>() == "#0099"
                })?;
                row.get(index.checked_sub(2)?).map(|cell| cell.fg)
            });
        assert_eq!(boss_label_color, Some(boss_color()));
        assert!(rendered.contains("LOOT CONTAINERS  1"));
        assert!(rendered.contains("Military Cra"));
        assert!(rendered.contains("Airdrop"));
        app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE))
            .unwrap();
        assert!(matches!(app.mode, Mode::ConfirmPunisher));
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .unwrap();
        app.handle_key(KeyEvent::new(KeyCode::Char('B'), KeyModifiers::SHIFT))
            .unwrap();
        assert!(matches!(app.mode, Mode::ConfirmBogeyman));
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .unwrap();
        app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE))
            .unwrap();
        assert!(matches!(app.mode, Mode::ConfirmAirdrop));
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE))
            .unwrap();
        assert!(matches!(app.mode, Mode::Normal));
    }

    #[test]
    fn vision_cone_samples_follow_world_heading_and_range() {
        let samples = vision_cone_samples([0.0, 2.0, 0.0], 0.0, 10.0, 60.0);
        assert_eq!(samples.len(), 41);
        assert!(
            samples
                .iter()
                .all(|point| (point[0].hypot(point[2]) - 10.0).abs() < 1e-9
                    || point[0].hypot(point[2]) < 10.0)
        );
        assert!(
            samples
                .iter()
                .any(|point| point[0].abs() < 1e-9 && (point[2] + 10.0).abs() < 1e-9)
        );
        assert_eq!(elevation_indicator(0.9), None);
        assert_eq!(elevation_indicator(1.0), Some('↑'));
        assert_eq!(elevation_indicator(-2.0), Some('↓'));
        assert_eq!(loot_elevation_indicator(1.9), None);
        assert_eq!(loot_elevation_indicator(2.0), Some('↑'));
    }

    #[test]
    fn gunshot_marker_fades_smoothly_to_the_scope_background() {
        use std::time::Duration;

        assert_eq!(shot_color(Duration::ZERO), Color::Rgb(255, 123, 63));
        let half = shot_color(Duration::from_millis(2500));
        assert_eq!(half, Color::Rgb(129, 65, 37));
        assert_eq!(shot_color(Duration::from_secs(5)), background());
        assert_eq!(shot_color(Duration::from_secs(9)), background());
    }

    #[test]
    fn tactical_palette_stays_dark_and_cool() {
        assert_eq!(background(), Color::Rgb(3, 7, 11));
        assert_eq!(surface(), Color::Rgb(7, 16, 23));
        assert_eq!(super::accent(), Color::Rgb(83, 181, 196));
        assert_eq!(super::selection_bg(), Color::Rgb(18, 53, 66));
    }

    #[test]
    fn shift_tab_moves_to_the_previous_panel() {
        let mut app = test_app();
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT))
            .unwrap();
        assert_eq!(app.panel, Panel::Backups);
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT))
            .unwrap();
        assert_eq!(app.panel, Panel::Radar);
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))
            .unwrap();
        assert_eq!(app.panel, Panel::Backups);
    }

    #[test]
    fn backup_tab_lists_only_selected_save_and_requires_typed_restore() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("rtv-toolkit-tab-backups-{unique}"));
        fs::create_dir(&dir).unwrap();
        let save = dir.join("Character.tres");
        fs::write(&save, TEST_CHARACTER).unwrap();
        let selected = dir.join("Character.tres.rtvbak.100");
        fs::write(
            &selected,
            TEST_CHARACTER.replace("health = 100.0", "health = 74.0"),
        )
        .unwrap();
        fs::write(dir.join("World.tres.rtvbak.101"), TEST_CHARACTER).unwrap();
        let mut app = App::new(
            CharacterDocument::load(&save).unwrap(),
            Catalog::load().unwrap(),
        )
        .unwrap();
        app.handle_key(KeyEvent::new(KeyCode::Char('5'), KeyModifiers::NONE))
            .unwrap();
        assert_eq!(app.panel, Panel::Backups);
        assert_eq!(app.backups.len(), 1);
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("SAVE BACKUPS"));
        assert!(rendered.contains("BACKUP DETAILS"));
        app.handle_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT))
            .unwrap();
        assert!(matches!(app.mode, Mode::ConfirmRestore { .. }));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .unwrap();
        assert_eq!(fs::read_to_string(&save).unwrap(), TEST_CHARACTER);
        for key in "RESTORE".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(key), KeyModifiers::SHIFT))
                .unwrap();
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .unwrap();
        assert!(matches!(app.mode, Mode::Normal));
        assert_eq!(app.document.stat("health").as_deref(), Some("74.0"));
        assert_eq!(app.backups.len(), 2);
        assert!(selected.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn backup_tab_refuses_unsaved_edits_and_dates_are_utc() {
        let mut app = test_app();
        app.document.set_vital("health", 81.0).unwrap();
        app.switch_panel(Panel::Backups);
        app.prepare_restore();
        assert!(app.status.contains("Unsaved edits"));
        assert!(matches!(app.mode, Mode::Normal));
        assert_eq!(backup_time(UNIX_EPOCH), "1970-01-01 00:00 UTC");
        assert_eq!(
            backup_time(UNIX_EPOCH + Duration::from_secs(1_790_985_600)),
            "2026-10-03 00:00 UTC"
        );
    }

    #[test]
    fn character_schematic_selection_drives_equipment_details() {
        let mut app = test_app();
        assert_eq!(app.panel, Panel::Character);
        assert_eq!(app.equip_selected, 0);
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .unwrap();
        assert_eq!(app.equip_selected, 1);
    }

    #[test]
    fn situation_appears_above_vitals_without_internal_data() {
        let app = test_app();
        let rendered = app
            .character_schematic_lines(TEST_SCHEMATIC_WIDTH)
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(rendered.find("SITUATION").unwrap() < rendered.find("MENTAL").unwrap());
        assert!(rendered.find("HANDS").unwrap() < rendered.find("BELT").unwrap());
        assert!(rendered.find("BELT").unwrap() < rendered.find("LEGS").unwrap());
        assert!(rendered.find("LEGS").unwrap() < rendered.find("FEET").unwrap());
        assert!(rendered.contains("HEALTH"));
        assert!(rendered.contains("THREATS"));
        assert!(!rendered.contains("RESERVES"));
        assert!(!rendered.contains("Character.tres"));
    }

    #[test]
    fn dashboard_has_distinct_equipment_and_inventory_readings() {
        let app = test_app();
        let rendered = app
            .character_schematic_lines(TEST_SCHEMATIC_WIDTH)
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(rendered.contains("[2] EQUIPMENT"));
        assert!(rendered.contains("SLOTS"));
        assert!(rendered.contains("AVG"));
        assert!(rendered.contains("[3] INVENTORY"));
        assert!(rendered.contains("ITEMS"));
        assert!(rendered.contains("CELLS"));
    }

    #[test]
    fn character_schematic_uses_the_available_width_without_overflow() {
        let app = test_app();
        for width in [TEST_SCHEMATIC_WIDTH, 116] {
            let lines = app.character_schematic_lines(width);
            assert!(lines.iter().all(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.chars().count())
                    .sum::<usize>()
                    == width
            }));
        }
    }

    #[test]
    fn character_vital_core_has_consistent_frame_edges() {
        let app = test_app();
        let lines = app.character_schematic_lines(TEST_SCHEMATIC_WIDTH);
        for line in &lines[4..17] {
            let center = line.spans[2].content.as_ref();
            assert_eq!(center.chars().count(), 28);
            assert!(matches!(center.chars().next(), Some('╭' | '│' | '╰')));
            assert!(matches!(center.chars().last(), Some('╮' | '│' | '╯')));
        }
    }

    #[test]
    fn inventory_shows_stored_condition_for_every_item() {
        let catalog = Catalog::load().unwrap();
        let magazine = catalog
            .get("res://Items/Weapons/M78/M78_Magazine.tres")
            .unwrap()
            .clone();
        assert!(!magazine.show_condition);
        let mut document = CharacterDocument::from_text(TEST_CHARACTER).unwrap();
        document.add_item(&magazine, &catalog).unwrap();
        let mut app = App::new(document, catalog).unwrap();
        app.panel = Panel::Inventory;
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("cond ["));
        assert!(rendered.contains("100%"));
    }

    #[test]
    fn character_callouts_are_mouse_selectable() {
        let mut app = test_app();
        app.equip_selected = 1;
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let primary = app
            .character_slot_rects
            .iter()
            .find_map(|(index, rect)| (*index == 0).then_some(*rect))
            .unwrap();
        app.handle_mouse(&MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: primary.x,
            row: primary.y,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.equip_selected, 0);
    }

    #[test]
    fn mouse_wheel_and_scrolled_click_follow_visible_rows() {
        let mut app = test_app();
        app.panel = Panel::Equipment;
        app.equip_selected = 1;
        app.handle_mouse(&MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.equip_selected, 0);
        app.handle_mouse(&MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.equip_selected, 1);

        app.equip_selected = 0;
        app.equipment_list_offset = 1;
        app.list_rect = Rect::new(0, 0, 40, 4);
        app.handle_mouse(&MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });
        assert_eq!(app.equip_selected, 1);
    }

    #[test]
    fn equipped_weapon_amount_is_capped_by_magazine_size() {
        let mut app = test_app();
        app.panel = Panel::Equipment;
        app.edit_equipment_amount(21);
        assert_eq!(app.selected_equipment_item().unwrap().amount, 18);
        assert!(app.status.contains("maximum of 20"));
    }

    #[test]
    fn refresh_reloads_world_state() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("rtv-toolkit-world-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        let character_path = dir.join("Character.tres");
        let world_path = dir.join("World.tres");
        fs::write(&character_path, TEST_CHARACTER).unwrap();
        fs::write(
            &world_path,
            "[gd_resource format=3]\n\n[resource]\nday = 1\ntime = 60\n",
        )
        .unwrap();
        let mut app = App::new(
            CharacterDocument::load(&character_path).unwrap(),
            Catalog::load().unwrap(),
        )
        .unwrap();
        assert_eq!(app.world.as_ref().unwrap().day, 1);
        fs::write(
            &world_path,
            "[gd_resource format=3]\n\n[resource]\nday = 2\ntime = 120\n",
        )
        .unwrap();
        app.refresh().unwrap();
        assert_eq!(app.world.as_ref().unwrap().day, 2);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn formats_conditions_on_the_games_0_to_100_scale() {
        assert_eq!(percent(100.0), "100%");
        assert_eq!(percent(97.0), "97%");
        assert_eq!(percent(97.5), "97.5%");
        assert_eq!(percent(99.999), "100%");
        assert_eq!(percent(0.0), "0%");
    }

    #[test]
    fn renders_separated_instrument_meters_with_half_tick_precision() {
        assert_eq!(condition_pattern(100.0), "[▮ ▮ ▮ ▮ ▮ ▮ ▮ ▮ ▮ ▮]");
        assert_eq!(filled_count(&condition_pattern(57.5)), 5);
        assert!(condition_pattern(57.5).contains('▌'));
        assert_eq!(filled_count(&condition_pattern(12.0)), 1);
        assert_eq!(condition_pattern(0.0), "[▯ ▯ ▯ ▯ ▯ ▯ ▯ ▯ ▯ ▯]");
    }

    #[test]
    fn ammunition_display_uses_one_indicator_per_round() {
        let rendered = super::ammunition_lines(17, 20)
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert_eq!(rendered.matches('●').count(), 17);
        assert_eq!(rendered.matches('○').count(), 3);
    }

    #[test]
    fn colors_bars_by_severity_tiers() {
        assert_eq!(condition_tiers(90.0).0, good());
        assert_eq!(condition_tiers(50.0).0, warning());
        assert_eq!(condition_tiers(10.0).0, danger());
    }

    #[test]
    fn only_weapons_get_chamber_details() {
        let catalog = Catalog::load().unwrap();
        let weapon = catalog.get("res://Items/Weapons/M78/M78.tres");
        assert!(item_is_weapon(weapon, "res://Items/Weapons/M78/M78.tres"));

        let knife = catalog.get("res://Items/Knives/Jaeger_140/Jaeger_140.tres");
        assert!(!item_is_weapon(
            knife,
            "res://Items/Knives/Jaeger_140/Jaeger_140.tres"
        ));

        let jeans = catalog.get("res://Items/Clothing/Jeans_Black/Jeans_Black.tres");
        assert!(!item_is_weapon(
            jeans,
            "res://Items/Clothing/Jeans_Black/Jeans_Black.tres"
        ));

        // Unlisted items fall back to the path name: a game equipment
        // weapon path still counts as a weapon, pants still do not.
        assert!(item_is_weapon(None, "res://res/game/equip/weapon_m78.xml"));
        assert!(!item_is_weapon(
            None,
            "res://res/game/equip/pants_jeans.xml"
        ));
    }

    #[test]
    fn stats_book_covers_weapons_with_real_values() {
        use crate::catalog::WeaponStatsBook;
        let book = WeaponStatsBook::load().unwrap();
        let m78 = book.get("res://Items/Weapons/M78/M78.tres").unwrap();
        assert_eq!(m78.damage, Some(50.0));
        assert_eq!(m78.penetration, Some(4.0));
        assert_eq!(m78.rpm, Some(667.0));
        assert_eq!(m78.magazine_size, Some(20.0));
        assert_eq!(m78.caliber.as_deref(), Some(".308"));
        assert!(
            book.get("res://Items/Weapons/Nonexistent/None.tres")
                .is_none()
        );
    }

    #[test]
    fn armory_binds_stats_to_the_equipped_primary() {
        let document = CharacterDocument::from_text(
            r#"[gd_resource type="Resource" script_class="CharacterSave" format=3]

[ext_resource type="Script" path="res://Scripts/SlotData.gd" id="1"]
[ext_resource type="Script" path="res://Scripts/ItemData.gd" id="2"]
[ext_resource type="Resource" path="res://Items/Weapons/M78/M78.tres" id="3"]
[ext_resource type="Script" path="res://Scripts/CharacterSave.gd" id="5"]

[sub_resource type="Resource" id="Resource_eqpri"]
script = ExtResource("1")
itemData = ExtResource("3")
condition = 93
amount = 18
chamber = true
casing = false
slot = "Primary"

[sub_resource type="Resource" id="Resource_eqsec"]
script = ExtResource("1")
condition = 100
amount = 0
chamber = false
casing = false
slot = "Secondary"

[resource]
script = ExtResource("5")
health = 100.0
energy = 100.0
hydration = 100.0
temperature = 100.0
mental = 100.0
inventory = Array[ExtResource("2")]([])
equipment = Array[ExtResource("2")]([SubResource("Resource_eqpri"), SubResource("Resource_eqsec")])
catalog = Array[ExtResource("2")]([])
"#,
        )
        .unwrap();
        let catalog = Catalog::load().unwrap();
        let app = super::App::new(document, catalog).unwrap();
        let armory: String = app
            .armory_lines()
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(armory.contains("M78"));
        assert!(armory.contains("Rifle"));
        assert!(armory.contains(".308"));
        assert!(armory.contains("DMG 50"));
        assert!(armory.contains("667"));
        assert!(armory.contains("MAG 18/20"));
        assert!(armory.contains("CHAMBER LOADED"));
    }
}
