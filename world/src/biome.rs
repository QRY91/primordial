use serde::Serialize;

/// Biome classification — a label derived from climate, not a constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum BiomeType {
    Tropical,
    Desert,
    TemperateForest,
    Grassland,
    Tundra,
    Ice,
}

impl BiomeType {
    /// Classify from current temperature and moisture.
    pub fn classify(temperature: f64, moisture: f64) -> Self {
        if temperature > 20.0 {
            if moisture > 0.5 {
                BiomeType::Tropical
            } else {
                BiomeType::Desert
            }
        } else if temperature > 5.0 {
            if moisture > 0.4 {
                BiomeType::TemperateForest
            } else {
                BiomeType::Grassland
            }
        } else if moisture > 0.3 {
            BiomeType::Tundra
        } else {
            BiomeType::Ice
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            BiomeType::Tropical => "tropical",
            BiomeType::Desert => "desert",
            BiomeType::TemperateForest => "temperate_forest",
            BiomeType::Grassland => "grassland",
            BiomeType::Tundra => "tundra",
            BiomeType::Ice => "ice",
        }
    }
}

/// Resource productivity as a function of temperature and moisture.
/// Peaks at ~25C with high moisture. Range: 0.0 to ~1.0.
pub fn productivity(temperature: f64, moisture: f64) -> f64 {
    let temp_factor = 1.0 - ((temperature - 25.0) / 40.0).powi(2);
    (temp_factor.max(0.0) * moisture).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tropical_classification() {
        assert_eq!(BiomeType::classify(30.0, 0.8), BiomeType::Tropical);
    }

    #[test]
    fn desert_classification() {
        assert_eq!(BiomeType::classify(35.0, 0.2), BiomeType::Desert);
    }

    #[test]
    fn ice_classification() {
        assert_eq!(BiomeType::classify(-10.0, 0.1), BiomeType::Ice);
    }

    #[test]
    fn productivity_peaks_warm_wet() {
        let tropical = productivity(25.0, 1.0);
        let desert = productivity(25.0, 0.1);
        let ice = productivity(-20.0, 0.1);
        assert!(tropical > desert);
        assert!(desert > ice);
        assert!(tropical > 0.9);
    }

    #[test]
    fn productivity_never_negative() {
        for t in [-50, -20, 0, 15, 25, 40, 60] {
            for m in [0, 25, 50, 75, 100] {
                let p = productivity(t as f64, m as f64 / 100.0);
                assert!(p >= 0.0, "productivity({t}, {m}) = {p}");
            }
        }
    }
}
