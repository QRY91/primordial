use rand::Rng;

/// A single cell in the climate grid.
pub struct ClimateCell {
    pub latitude: f64,         // -1.0 (south) to 1.0 (north)
    pub base_temperature: f64, // Celsius, without seasonal/weather
    pub base_moisture: f64,    // 0.0 to 1.0
    pub temperature: f64,      // current (season + weather applied)
    pub moisture: f64,         // current
    pub adjacency: Vec<usize>, // neighbor cell indices
}

/// 2D grid of climate cells. Rows = latitude, columns = longitude (wrapping).
pub struct ClimateGrid {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<ClimateCell>,
}

impl ClimateGrid {
    /// Build a grid from physical parameters.
    /// Temperature gradient from equator to poles, moisture from hydrosphere.
    pub fn new(
        width: usize,
        height: usize,
        equator_temp: f64,
        hydrosphere: f64,
    ) -> Self {
        let mut cells = Vec::with_capacity(width * height);

        for y in 0..height {
            for x in 0..width {
                let lat = if height > 1 {
                    1.0 - 2.0 * y as f64 / (height - 1) as f64
                } else {
                    0.0
                };

                // Temperature: warm at equator, cold at poles
                let base_temp = equator_temp * (1.0 - 0.5 * lat * lat)
                    - 15.0 * lat.abs();

                // Moisture: varies with latitude and longitude
                let lon_factor = if width > 1 {
                    let angle = 2.0 * std::f64::consts::PI * x as f64 / width as f64;
                    0.6 + 0.4 * angle.cos()
                } else {
                    1.0
                };
                let base_moisture =
                    (hydrosphere * lon_factor * (1.0 - 0.4 * lat * lat)).clamp(0.05, 1.0);

                // 4-connected adjacency, longitude wraps
                let idx = y * width + x;
                let mut adj = Vec::new();
                if width > 1 {
                    adj.push(y * width + (x + width - 1) % width);
                    adj.push(y * width + (x + 1) % width);
                }
                if y > 0 {
                    adj.push((y - 1) * width + x);
                }
                if y + 1 < height {
                    adj.push((y + 1) * width + x);
                }
                // Remove self-loops (can happen with width=2)
                adj.retain(|&a| a != idx);

                cells.push(ClimateCell {
                    latitude: lat,
                    base_temperature: base_temp,
                    base_moisture: base_moisture,
                    temperature: base_temp,
                    moisture: base_moisture,
                    adjacency: adj,
                });
            }
        }

        Self {
            width,
            height,
            cells,
        }
    }

    /// Update temperature and moisture for current tick.
    pub fn tick(
        &mut self,
        current_tick: u64,
        season_period: u64,
        seasonal_amplitude: f64,
        weather_volatility: f64,
        rng: &mut impl Rng,
    ) {
        let phase =
            2.0 * std::f64::consts::PI * (current_tick as f64) / (season_period.max(1) as f64);
        let season_sin = phase.sin();

        for cell in &mut self.cells {
            // Season: northern hemisphere warms when sin > 0, southern opposite
            let seasonal = seasonal_amplitude * season_sin * cell.latitude;

            // Weather: random perturbation
            let temp_noise = weather_volatility * 5.0 * (rng.gen::<f64>() * 2.0 - 1.0);
            let moist_noise = weather_volatility * 0.1 * (rng.gen::<f64>() * 2.0 - 1.0);

            cell.temperature = cell.base_temperature + seasonal + temp_noise;
            cell.moisture = (cell.base_moisture + moist_noise).clamp(0.0, 1.0);
        }
    }

    pub fn num_cells(&self) -> usize {
        self.cells.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn single_cell_grid() {
        let grid = ClimateGrid::new(1, 1, 15.0, 0.7);
        assert_eq!(grid.num_cells(), 1);
        assert!(grid.cells[0].adjacency.is_empty());
        assert!((grid.cells[0].latitude - 0.0).abs() < 0.01);
    }

    #[test]
    fn equator_warmer_than_poles() {
        let grid = ClimateGrid::new(6, 6, 15.0, 0.7);
        let equator_row = 3; // middle-ish
        let pole_row = 0;
        let eq_temp = grid.cells[equator_row * 6].base_temperature;
        let pole_temp = grid.cells[pole_row * 6].base_temperature;
        assert!(eq_temp > pole_temp, "equator={eq_temp} should be warmer than pole={pole_temp}");
    }

    #[test]
    fn seasonal_variation() {
        let mut grid = ClimateGrid::new(4, 4, 15.0, 0.7);
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        // Record temperatures at two seasonal extremes
        grid.tick(0, 1000, 15.0, 0.0, &mut rng); // sin(0) = 0
        let t0 = grid.cells[0].temperature; // north pole

        grid.tick(250, 1000, 15.0, 0.0, &mut rng); // sin(pi/2) = 1
        let t250 = grid.cells[0].temperature;

        // North pole (lat=1) should be warmer at tick 250
        assert!(t250 > t0, "north pole should warm in summer: t0={t0}, t250={t250}");
    }

    #[test]
    fn adjacency_wraps_longitude() {
        let grid = ClimateGrid::new(4, 4, 15.0, 0.7);
        // Cell (0,0) should be adjacent to cell (3,0) via wrap
        let adj = &grid.cells[0].adjacency;
        assert!(adj.contains(&3), "cell 0 should be adjacent to cell 3 (wrap)");
    }
}
