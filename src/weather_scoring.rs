//! `meteo-types` integration for weather observation scoring.

use meteo_types::{SensorReferencePair, WeatherObservation};
use serde::{Deserialize, Serialize};

use crate::{ForecastAccuracyMetrics, Pair, ScoreError};

/// Per-variable scores computed from weather observations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(export, export_to = "bindings/"))]
pub struct WeatherVariableScores {
    pub temperature: Option<ForecastAccuracyMetrics>,
    pub humidity: Option<ForecastAccuracyMetrics>,
    pub pressure: Option<ForecastAccuracyMetrics>,
    pub wind_speed: Option<ForecastAccuracyMetrics>,
    pub wind_direction: Option<ForecastAccuracyMetrics>,
    pub precipitation: Option<ForecastAccuracyMetrics>,
    pub dewpoint: Option<ForecastAccuracyMetrics>,
    pub visibility: Option<ForecastAccuracyMetrics>,
}

impl From<SensorReferencePair> for Pair {
    fn from(value: SensorReferencePair) -> Self {
        Self::new(value.sensor_value, value.reference_value)
    }
}

fn collect_pairs<F>(
    forecast: &[WeatherObservation],
    observed: &[WeatherObservation],
    selector: F,
) -> Vec<Pair>
where
    F: Fn(&WeatherObservation) -> Option<f64>,
{
    forecast
        .iter()
        .zip(observed.iter())
        .filter_map(|(forecast_obs, observed_obs)| {
            let predicted = selector(forecast_obs)?;
            let actual = selector(observed_obs)?;
            Some(Pair::new(predicted, actual))
        })
        .collect()
}

fn score_variable<F>(
    forecast: &[WeatherObservation],
    observed: &[WeatherObservation],
    selector: F,
) -> Option<ForecastAccuracyMetrics>
where
    F: Fn(&WeatherObservation) -> Option<f64>,
{
    let pairs = collect_pairs(forecast, observed, selector);
    if pairs.is_empty() {
        None
    } else {
        Some(ForecastAccuracyMetrics::from_pairs(&pairs))
    }
}

/// Score weather forecasts against observations for each variable independently.
pub fn score_weather_observations(
    forecast: &[WeatherObservation],
    observed: &[WeatherObservation],
) -> Result<WeatherVariableScores, ScoreError> {
    if forecast.len() != observed.len() {
        return Err(ScoreError::LengthMismatch {
            expected: forecast.len(),
            got: observed.len(),
        });
    }

    Ok(WeatherVariableScores {
        temperature: score_variable(forecast, observed, |obs| obs.temperature_c),
        humidity: score_variable(forecast, observed, |obs| obs.humidity_percent),
        pressure: score_variable(forecast, observed, |obs| obs.pressure_hpa),
        wind_speed: score_variable(forecast, observed, |obs| obs.wind_speed_ms),
        wind_direction: score_variable(forecast, observed, |obs| obs.wind_direction_deg),
        precipitation: score_variable(forecast, observed, |obs| obs.precipitation_mm),
        dewpoint: score_variable(forecast, observed, |obs| obs.dewpoint_c),
        visibility: score_variable(forecast, observed, |obs| obs.visibility_m),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(
        temperature_c: Option<f64>,
        humidity_percent: Option<f64>,
        wind_speed_ms: Option<f64>,
    ) -> WeatherObservation {
        WeatherObservation {
            temperature_c,
            humidity_percent,
            wind_speed_ms,
            ..WeatherObservation::default()
        }
    }

    #[test]
    fn score_weather_observations_rejects_length_mismatch() {
        let err = score_weather_observations(&[WeatherObservation::default()], &[]).unwrap_err();
        assert_eq!(
            err,
            ScoreError::LengthMismatch {
                expected: 1,
                got: 0
            }
        );
    }

    #[test]
    fn score_weather_observations_skips_missing_values() {
        let forecast = [
            obs(Some(20.0), Some(60.0), Some(5.0)),
            obs(None, Some(55.0), Some(4.0)),
        ];
        let observed = [
            obs(Some(19.0), Some(62.0), Some(5.0)),
            obs(Some(18.0), None, Some(6.0)),
        ];
        let scores = score_weather_observations(&forecast, &observed).unwrap();
        assert!(scores.temperature.is_some());
        assert!(scores.humidity.is_some());
        assert!(scores.wind_speed.is_some());
        assert!(scores.pressure.is_none());
    }

    #[test]
    fn sensor_reference_pair_converts_to_pair() {
        let pair: Pair = SensorReferencePair::new(10.5, 9.5).into();
        assert_eq!(pair, Pair::new(10.5, 9.5));
    }

    #[test]
    fn score_weather_observations_handles_realistic_temperature_series() {
        let forecast = [
            obs(Some(20.0), Some(60.0), None),
            obs(Some(21.5), Some(58.0), None),
            obs(Some(19.8), Some(64.0), None),
        ];
        let observed = [
            obs(Some(19.5), Some(62.0), None),
            obs(Some(21.0), Some(56.0), None),
            obs(Some(20.0), Some(63.0), None),
        ];
        let scores = score_weather_observations(&forecast, &observed).unwrap();
        let temperature = scores.temperature.unwrap();
        assert!(temperature.mae >= 0.0);
        assert!(temperature.skill_score <= 1.0);
        assert!(scores.humidity.is_some());
    }
}
