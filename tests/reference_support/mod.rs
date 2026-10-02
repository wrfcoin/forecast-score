use forecast_score::{EnsembleForecast, Pair};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ProbabilityData {
    pub rows: Vec<(f64, u64, u64)>,
    pub expected: serde_json::Value,
}

impl ProbabilityData {
    pub fn pairs(&self) -> Vec<Pair> {
        self.rows
            .iter()
            .flat_map(|&(p, count, events)| {
                assert!(events <= count);
                (0..count).map(move |i| Pair::new(p, if i < events { 1.0 } else { 0.0 }))
            })
            .collect()
    }
}

#[derive(Deserialize)]
pub struct AnalyticData {
    pub ensembles: Vec<EnsembleForecast>,
    pub expected: serde_json::Value,
}

impl AnalyticData {
    pub fn pairs(&self) -> Vec<Pair> {
        self.ensembles
            .iter()
            .map(|f| {
                Pair::new(
                    f.members.iter().sum::<f64>() / f.members.len() as f64,
                    f.observed,
                )
            })
            .collect()
    }
}

pub fn probabilities() -> [(&'static str, ProbabilityData); 2] {
    [
        (
            "tampere24",
            serde_json::from_str(include_str!("../reference_data/verification_pop_24h.json"))
                .unwrap(),
        ),
        (
            "tampere48",
            serde_json::from_str(include_str!("../reference_data/verification_pop_48h.json"))
                .unwrap(),
        ),
    ]
}

pub fn analytic() -> AnalyticData {
    serde_json::from_str(include_str!("../reference_data/verification_analytic.json")).unwrap()
}
