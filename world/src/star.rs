/// A main-sequence star. Mass determines luminosity and habitable zone.
pub struct Star {
    pub mass: f64, // solar masses
}

impl Star {
    pub fn new(mass: f64) -> Self {
        Self { mass: mass.max(0.1) }
    }

    /// Main-sequence luminosity: L ~ M^3.5
    pub fn luminosity(&self) -> f64 {
        self.mass.powf(3.5)
    }

    /// Inner edge of habitable zone in AU.
    pub fn habitable_zone_inner(&self) -> f64 {
        self.luminosity().sqrt() * 0.75
    }

    /// Outer edge of habitable zone in AU.
    pub fn habitable_zone_outer(&self) -> f64 {
        self.luminosity().sqrt() * 1.8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solar_luminosity() {
        let sun = Star::new(1.0);
        assert!((sun.luminosity() - 1.0).abs() < 0.01);
    }

    #[test]
    fn habitable_zone_contains_earth() {
        let sun = Star::new(1.0);
        assert!(sun.habitable_zone_inner() < 1.0);
        assert!(sun.habitable_zone_outer() > 1.0);
    }

    #[test]
    fn brighter_star_wider_zone() {
        let small = Star::new(0.5);
        let large = Star::new(2.0);
        assert!(large.habitable_zone_outer() > small.habitable_zone_outer());
    }
}
