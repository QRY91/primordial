use crate::star::Star;

/// A planet with orbital and surface parameters.
pub struct Planet {
    pub orbital_radius: f64, // AU
    pub axial_tilt: f64,     // degrees (0-45)
    pub hydrosphere: f64,    // fraction of surface covered by water (0-1)
}

impl Planet {
    pub fn new(orbital_radius: f64, axial_tilt: f64, hydrosphere: f64) -> Self {
        Self {
            orbital_radius: orbital_radius.max(0.01),
            axial_tilt: axial_tilt.clamp(0.0, 45.0),
            hydrosphere: hydrosphere.clamp(0.0, 1.0),
        }
    }

    /// Equilibrium surface temperature in Celsius.
    /// Earth (L=1, r=1) -> ~15C.
    pub fn equilibrium_temp(&self, star: &Star) -> f64 {
        let flux = star.luminosity() / (self.orbital_radius * self.orbital_radius);
        288.0 * flux.powf(0.25) - 273.15
    }

    /// Seasonal temperature amplitude in degrees C.
    /// Earth (23.5 deg tilt) -> ~15C swing.
    pub fn seasonal_amplitude(&self) -> f64 {
        self.axial_tilt / 23.5 * 15.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earth_like_temperature() {
        let star = Star::new(1.0);
        let earth = Planet::new(1.0, 23.5, 0.7);
        let temp = earth.equilibrium_temp(&star);
        assert!((temp - 14.85).abs() < 1.0, "Earth-like should be ~15C, got {temp}");
    }

    #[test]
    fn closer_planet_is_hotter() {
        let star = Star::new(1.0);
        let close = Planet::new(0.7, 23.5, 0.7);
        let far = Planet::new(1.5, 23.5, 0.7);
        assert!(close.equilibrium_temp(&star) > far.equilibrium_temp(&star));
    }

    #[test]
    fn no_tilt_no_seasons() {
        let p = Planet::new(1.0, 0.0, 0.7);
        assert!((p.seasonal_amplitude() - 0.0).abs() < 0.01);
    }
}
