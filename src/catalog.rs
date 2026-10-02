use std::collections::HashMap;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct CatalogItem {
    pub category: String,
    pub id: String,
    pub name: String,
    pub path: String,
    pub width: usize,
    pub height: usize,
    pub stackable: bool,
    pub show_condition: bool,
    pub show_amount: bool,
    pub default_amount: i64,
    pub max_amount: Option<i64>,
}

impl CatalogItem {
    pub fn size(&self, rotated: bool) -> (usize, usize) {
        if rotated {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }

    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || self.name.to_lowercase().contains(&query)
            || self.id.to_lowercase().contains(&query)
            || self.category.to_lowercase().contains(&query)
            || self.path.to_lowercase().contains(&query)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct WeaponStats {
    pub path: String,
    pub name: String,
    pub weapon_type: Option<String>,
    pub action: Option<String>,
    pub caliber: Option<String>,
    pub damage: Option<f64>,
    pub penetration: Option<f64>,
    pub magazine_size: Option<f64>,
    pub rpm: Option<f64>,
    pub kick: Option<f64>,
    pub weight: Option<f64>,
    pub value: Option<f64>,
    pub rarity: Option<f64>,
}

/// Stats for all weapons in the current build, keyed by resource path.
#[derive(Debug)]
pub struct WeaponStatsBook {
    stats: Vec<WeaponStats>,
    by_path: HashMap<String, usize>,
}

impl WeaponStatsBook {
    pub fn load() -> Result<Self> {
        let stats: Vec<WeaponStats> =
            serde_json::from_str(include_str!("../data/weapon-stats.json"))
                .context("the bundled weapon stats are invalid")?;
        let by_path = stats
            .iter()
            .enumerate()
            .map(|(index, stats)| (stats.path.clone(), index))
            .collect();
        Ok(Self { stats, by_path })
    }

    pub fn get(&self, path: &str) -> Option<&WeaponStats> {
        self.by_path.get(path).map(|index| &self.stats[*index])
    }
}

#[derive(Debug)]
pub struct Catalog {
    items: Vec<CatalogItem>,
    by_path: HashMap<String, usize>,
}

impl Catalog {
    pub fn load() -> Result<Self> {
        let items: Vec<CatalogItem> = serde_json::from_str(include_str!("../data/items.json"))
            .context("the bundled item catalog is invalid")?;
        let by_path = items
            .iter()
            .enumerate()
            .map(|(index, item)| (item.path.clone(), index))
            .collect();
        Ok(Self { items, by_path })
    }

    pub fn items(&self) -> &[CatalogItem] {
        &self.items
    }

    pub fn get(&self, path: &str) -> Option<&CatalogItem> {
        self.by_path.get(path).map(|index| &self.items[*index])
    }

    pub fn filtered_indices(&self, query: &str) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| item.matches(query).then_some(index))
            .collect()
    }
}
