use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarMode {
    Track,
    Acoustic,
    Hybrid,
}

impl RadarMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Track => Self::Acoustic,
            Self::Acoustic => Self::Hybrid,
            Self::Hybrid => Self::Track,
        }
    }

    pub fn shows_ai(self) -> bool {
        matches!(self, Self::Track | Self::Hybrid)
    }

    pub fn shows_shots(self) -> bool {
        matches!(self, Self::Acoustic | Self::Hybrid)
    }
}

impl fmt::Display for RadarMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Track => "TRACK",
            Self::Acoustic => "ACOUSTIC",
            Self::Hybrid => "HYBRID",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectedContact {
    /// Metres to the player's right; negative values are left.
    pub right: f64,
    /// Metres ahead of the player; negative values are behind.
    pub forward: f64,
    pub distance: f64,
}

pub fn project_contact(
    player_position: [f64; 3],
    player_heading_degrees: f64,
    target_position: [f64; 3],
    range_metres: f64,
) -> Option<ProjectedContact> {
    if !range_metres.is_finite() || range_metres <= 0.0 {
        return None;
    }
    let east = target_position[0] - player_position[0];
    let north = -(target_position[2] - player_position[2]);
    let distance = east.hypot(north);
    if !distance.is_finite() || distance > range_metres {
        return None;
    }
    let heading = player_heading_degrees.to_radians();
    let (sin_heading, cos_heading) = heading.sin_cos();
    Some(ProjectedContact {
        right: east * cos_heading - north * sin_heading,
        forward: east * sin_heading + north * cos_heading,
        distance,
    })
}

pub fn contact_cell(
    contact: ProjectedContact,
    range_metres: f64,
    radius_x: f64,
    radius_y: f64,
) -> (i32, i32) {
    let column = (contact.right / range_metres * radius_x).round() as i32;
    let row = (-contact.forward / range_metres * radius_y).round() as i32;
    (column, row)
}

#[cfg(test)]
mod tests {
    use super::{RadarMode, contact_cell, project_contact};

    fn project(heading: f64, target: [f64; 3]) -> super::ProjectedContact {
        project_contact([0.0, 0.0, 0.0], heading, target, 100.0).unwrap()
    }

    #[test]
    fn projects_ai_directly_ahead_and_behind() {
        let ahead = project(0.0, [0.0, 0.0, -10.0]);
        assert!(ahead.right.abs() < 1e-9);
        assert!((ahead.forward - 10.0).abs() < 1e-9);
        let behind = project(0.0, [0.0, 0.0, 10.0]);
        assert!(behind.right.abs() < 1e-9);
        assert!((behind.forward + 10.0).abs() < 1e-9);
    }

    #[test]
    fn projects_ai_left_and_right() {
        let right = project(0.0, [10.0, 0.0, 0.0]);
        assert!((right.right - 10.0).abs() < 1e-9);
        let left = project(0.0, [-10.0, 0.0, 0.0]);
        assert!((left.right + 10.0).abs() < 1e-9);
    }

    #[test]
    fn rotates_world_coordinates_by_player_heading() {
        let ahead_when_facing_east = project(90.0, [10.0, 0.0, 0.0]);
        assert!(ahead_when_facing_east.right.abs() < 1e-9);
        assert!((ahead_when_facing_east.forward - 10.0).abs() < 1e-9);

        let north_when_facing_east = project(90.0, [0.0, 0.0, -10.0]);
        assert!((north_when_facing_east.right + 10.0).abs() < 1e-9);
        assert!(north_when_facing_east.forward.abs() < 1e-9);
    }

    #[test]
    fn clips_contacts_outside_radar_range() {
        assert!(project_contact([0.0; 3], 0.0, [0.0, 0.0, -100.01], 100.0).is_none());
        assert!(project_contact([0.0; 3], 0.0, [0.0, 50.0, -100.0], 100.0).is_some());
    }

    #[test]
    fn radar_modes_expose_only_the_intended_contacts() {
        assert!(RadarMode::Track.shows_ai());
        assert!(!RadarMode::Track.shows_shots());
        assert!(!RadarMode::Acoustic.shows_ai());
        assert!(RadarMode::Acoustic.shows_shots());
        assert!(RadarMode::Hybrid.shows_ai());
        assert!(RadarMode::Hybrid.shows_shots());
    }

    #[test]
    fn maps_ahead_to_the_top_of_the_radar() {
        let ahead = project(0.0, [0.0, 0.0, -100.0]);
        assert_eq!(contact_cell(ahead, 100.0, 20.0, 10.0), (0, -10));
    }
}
