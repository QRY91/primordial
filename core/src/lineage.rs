use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::genome::Genome;
use crate::organism::LineageId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LineageEventType {
    Emerged,
    Extinct,
    Snapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageEvent {
    pub event_type: LineageEventType,
    pub tick: u64,
    pub lineage_id: LineageId,
    pub parent_lineage_id: Option<LineageId>,
    pub genome_snapshot: u64,
    pub population_count: u32,
}

pub struct LineageTracker {
    next_lineage_id: LineageId,
    /// Maps lineage_id -> count of living organisms in that lineage
    active_lineages: HashMap<LineageId, u32>,
    /// Hamming distance threshold for forking a new lineage
    divergence_threshold: u32,
    /// Buffered events to flush to log
    pending_events: Vec<LineageEvent>,
}

impl LineageTracker {
    pub fn new(divergence_threshold: u32) -> Self {
        Self {
            next_lineage_id: 0,
            active_lineages: HashMap::new(),
            divergence_threshold,
            pending_events: Vec::new(),
        }
    }

    /// Allocate a fresh lineage ID and emit an Emerged event.
    pub fn create_lineage(
        &mut self,
        parent_lineage: Option<LineageId>,
        genome: &Genome,
        tick: u64,
    ) -> LineageId {
        let id = self.next_lineage_id;
        self.next_lineage_id += 1;
        self.active_lineages.insert(id, 0);
        self.pending_events.push(LineageEvent {
            event_type: LineageEventType::Emerged,
            tick,
            lineage_id: id,
            parent_lineage_id: parent_lineage,
            genome_snapshot: genome.0,
            population_count: 0,
        });
        id
    }

    /// Compare child to parent genome. If diverged enough, fork a new lineage.
    pub fn assign_lineage(
        &mut self,
        parent_genome: &Genome,
        child_genome: &Genome,
        parent_lineage: LineageId,
        tick: u64,
    ) -> LineageId {
        if parent_genome.distance(child_genome) > self.divergence_threshold {
            self.create_lineage(Some(parent_lineage), child_genome, tick)
        } else {
            parent_lineage
        }
    }

    /// Increment organism count for a lineage.
    pub fn record_birth(&mut self, lineage_id: LineageId) {
        *self.active_lineages.entry(lineage_id).or_insert(0) += 1;
    }

    /// Decrement organism count. Emits Extinct event if count reaches 0.
    pub fn record_death(&mut self, lineage_id: LineageId, genome: &Genome, tick: u64) {
        if let Some(count) = self.active_lineages.get_mut(&lineage_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.pending_events.push(LineageEvent {
                    event_type: LineageEventType::Extinct,
                    tick,
                    lineage_id,
                    parent_lineage_id: None,
                    genome_snapshot: genome.0,
                    population_count: 0,
                });
                self.active_lineages.remove(&lineage_id);
            }
        }
    }

    /// Emit snapshot events for all active lineages.
    pub fn snapshot(&mut self, tick: u64) {
        let events: Vec<LineageEvent> = self
            .active_lineages
            .iter()
            .map(|(&lineage_id, &count)| LineageEvent {
                event_type: LineageEventType::Snapshot,
                tick,
                lineage_id,
                parent_lineage_id: None,
                genome_snapshot: 0,
                population_count: count,
            })
            .collect();
        self.pending_events.extend(events);
    }

    /// Drain buffered events for flushing to log.
    pub fn drain_events(&mut self) -> Vec<LineageEvent> {
        std::mem::take(&mut self.pending_events)
    }

    pub fn active_lineage_count(&self) -> usize {
        self.active_lineages.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_lineage_on_divergence() {
        let mut tracker = LineageTracker::new(4);
        let parent_lineage = tracker.create_lineage(None, &Genome(0xFF), 0);
        tracker.record_birth(parent_lineage);

        // Child genome differs by 8 bits — exceeds threshold of 4
        let parent_genome = Genome(0x00);
        let child_genome = Genome(0xFF);
        let child_lineage =
            tracker.assign_lineage(&parent_genome, &child_genome, parent_lineage, 1);

        assert_ne!(child_lineage, parent_lineage);
        assert_eq!(tracker.active_lineage_count(), 2);
    }

    #[test]
    fn same_lineage_within_threshold() {
        let mut tracker = LineageTracker::new(8);
        let parent_lineage = tracker.create_lineage(None, &Genome(0x00), 0);
        tracker.record_birth(parent_lineage);

        // Child differs by only 1 bit
        let parent_genome = Genome(0x00);
        let child_genome = Genome(0x01);
        let child_lineage =
            tracker.assign_lineage(&parent_genome, &child_genome, parent_lineage, 1);

        assert_eq!(child_lineage, parent_lineage);
    }

    #[test]
    fn extinction_on_last_death() {
        let mut tracker = LineageTracker::new(8);
        let lineage = tracker.create_lineage(None, &Genome(0xAA), 0);
        tracker.record_birth(lineage);

        assert_eq!(tracker.active_lineage_count(), 1);
        tracker.record_death(lineage, &Genome(0xAA), 10);
        assert_eq!(tracker.active_lineage_count(), 0);

        let events = tracker.drain_events();
        let extinct_events: Vec<_> = events
            .iter()
            .filter(|e| matches!(e.event_type, LineageEventType::Extinct))
            .collect();
        assert_eq!(extinct_events.len(), 1);
        assert_eq!(extinct_events[0].tick, 10);
    }
}
