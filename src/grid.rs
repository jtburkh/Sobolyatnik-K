use std::collections::{HashMap, HashSet};

use crate::{catalog::Catalog, tres::InventoryItem};

pub const COLS: usize = 8;
pub const ROWS: usize = 13;
pub const CELL_SIZE: i32 = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    pub col: usize,
    pub row: usize,
    pub width: usize,
    pub height: usize,
    pub rotated: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ValidationReport {
    pub errors: Vec<String>,
    pub conflicting_ids: HashSet<String>,
    pub occupied_cells: usize,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn validate(items: &[InventoryItem], catalog: &Catalog) -> ValidationReport {
    let mut report = ValidationReport::default();
    let mut cells: HashMap<(usize, usize), &InventoryItem> = HashMap::new();
    let mut ids = HashSet::new();

    for item in items {
        if !ids.insert(item.resource_id.as_str()) {
            report.errors.push(format!(
                "inventory references {} more than once",
                item.resource_id
            ));
            report.conflicting_ids.insert(item.resource_id.clone());
        }

        let Some(meta) = catalog.get(&item.path) else {
            report.errors.push(format!(
                "{} has unknown item metadata: {}",
                item.resource_id, item.path
            ));
            report.conflicting_ids.insert(item.resource_id.clone());
            continue;
        };

        if item.x < 0 || item.y < 0 || item.x % CELL_SIZE != 0 || item.y % CELL_SIZE != 0 {
            report.errors.push(format!(
                "{} has a non-grid position ({}, {})",
                item.resource_id, item.x, item.y
            ));
            report.conflicting_ids.insert(item.resource_id.clone());
            continue;
        }

        let col = (item.x / CELL_SIZE) as usize;
        let row = (item.y / CELL_SIZE) as usize;
        let (width, height) = meta.size(item.rotated);
        if col + width > COLS || row + height > ROWS {
            report.errors.push(format!(
                "{} ({}) is outside the {}x{} inventory at ({}, {}), size {}x{}",
                item.resource_id, meta.name, COLS, ROWS, col, row, width, height
            ));
            report.conflicting_ids.insert(item.resource_id.clone());
            continue;
        }

        for y in row..row + height {
            for x in col..col + width {
                if let Some(other) = cells.insert((x, y), item) {
                    report.errors.push(format!(
                        "{} ({}) overlaps {} at cell ({}, {})",
                        item.resource_id, meta.name, other.resource_id, x, y
                    ));
                    report.conflicting_ids.insert(item.resource_id.clone());
                    report.conflicting_ids.insert(other.resource_id.clone());
                }
            }
        }
    }

    report.occupied_cells = cells.len();
    report
}

pub fn find_free_slot(items: &[InventoryItem], catalog: &Catalog, path: &str) -> Option<Placement> {
    // Refuse to claim a safe placement if any existing footprint is unknown or invalid.
    if !validate(items, catalog).is_valid() {
        return None;
    }

    let meta = catalog.get(path)?;
    let mut occupied = [[false; COLS]; ROWS];
    for item in items {
        let existing = catalog.get(&item.path)?;
        let col = (item.x / CELL_SIZE) as usize;
        let row = (item.y / CELL_SIZE) as usize;
        let (width, height) = existing.size(item.rotated);
        for cells in occupied.iter_mut().skip(row).take(height) {
            for cell in cells.iter_mut().skip(col).take(width) {
                *cell = true;
            }
        }
    }

    for rotated in [false, true] {
        if rotated && meta.width == meta.height {
            continue;
        }
        let (width, height) = meta.size(rotated);
        if width > COLS || height > ROWS {
            continue;
        }
        for row in 0..=ROWS - height {
            for col in 0..=COLS - width {
                let free = occupied
                    .iter()
                    .skip(row)
                    .take(height)
                    .all(|cells| cells.iter().skip(col).take(width).all(|used| !used));
                if free {
                    return Some(Placement {
                        col,
                        row,
                        width,
                        height,
                        rotated,
                    });
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, path: &str, col: i32, row: i32, rotated: bool) -> InventoryItem {
        InventoryItem {
            resource_id: id.into(),
            path: path.into(),
            x: col * CELL_SIZE,
            y: row * CELL_SIZE,
            rotated,
            amount: 0,
            condition: 100.0,
        }
    }

    #[test]
    fn detects_full_footprint_collisions() {
        let catalog = Catalog::load().unwrap();
        let armor = "res://Items/Armor/Armor_Plate_IV.tres"; // 2x2
        let bandage = "res://Items/Medical/Bandage/Bandage.tres"; // 1x1
        let report = validate(
            &[
                item("armor", armor, 0, 0, false),
                item("bandage", bandage, 1, 1, false),
            ],
            &catalog,
        );
        assert!(!report.is_valid());
        assert!(report.conflicting_ids.contains("armor"));
        assert!(report.conflicting_ids.contains("bandage"));
    }

    #[test]
    fn placement_uses_rotation_when_required() {
        let catalog = Catalog::load().unwrap();
        let bandage = "res://Items/Medical/Bandage/Bandage.tres";
        let mut items = Vec::new();
        // Fill every cell except a 2x1 opening at the end of the last row.
        for row in 0..ROWS {
            for col in 0..COLS {
                if row == ROWS - 1 && col >= COLS - 2 {
                    continue;
                }
                items.push(item(
                    &format!("i{col}_{row}"),
                    bandage,
                    col as i32,
                    row as i32,
                    false,
                ));
            }
        }
        let splint = "res://Items/Medical/Splint/Splint.tres"; // 1x2, rotates to 2x1
        let slot = find_free_slot(&items, &catalog, splint).unwrap();
        assert_eq!((slot.col, slot.row, slot.width, slot.height), (6, 12, 2, 1));
        assert!(slot.rotated);
    }
}
