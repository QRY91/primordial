use thiserror::Error;

#[derive(Error, Debug)]
pub enum PrimordialError {
    #[error("population extinct at tick {tick}")]
    PopulationExtinct { tick: u64 },

    #[error("invalid config: {msg}")]
    InvalidConfig { msg: String },

    #[error("resource underflow: requested {requested}, available {available}")]
    ResourceUnderflow { requested: f64, available: f64 },

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, PrimordialError>;
