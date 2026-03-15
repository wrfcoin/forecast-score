//! Blockchain types for forecast verification on the WRFCoin network.
//!
//! Contains [`ForecastArtifactManifest`] and [`ForecastVerificationReceipt`] structs
//! along with ed25519 signing infrastructure for deterministic forecast verification.

use ed25519_dalek::{Signature as Ed25519Signature, Verifier, VerifyingKey};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use thiserror::Error;

use super::ForecastAccuracyMetrics;

/// Version tag embedded in every canonical signing message.
const SIGNING_SCHEME_VERSION: &str = "wrfcoin-sign-v1";
/// Default chain identifier used in signing messages.
const DEFAULT_CHAIN_ID: &str = "wrfcoin-main";
/// Default network identifier used in signing messages.
const DEFAULT_NETWORK_ID: &str = "mainnet";

// ---------------------------------------------------------------------------
// ForecastArtifactManifest
// ---------------------------------------------------------------------------

/// Canonical representation of forecast inputs and outputs for deterministic verification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForecastArtifactManifest {
    /// Schema version for the manifest layout.
    pub schema_version: u32,
    /// Forecast model identifier.
    pub model_id: String,
    /// Hex-encoded digest of the forecast inputs.
    pub input_hash: String,
    /// Hex-encoded digest of the forecast outputs.
    pub output_hash: String,
    /// Unix timestamp at which the forecast artifacts were produced.
    pub produced_at: i64,
    /// Geographic region covered by the forecast.
    pub region: String,
    /// Forecast horizon in hours.
    pub horizon_hours: u32,
    /// Sorted list of input variable names.
    pub input_variables: Vec<String>,
    /// Sorted list of output variable names.
    pub output_variables: Vec<String>,
}

/// Validation errors for forecast artifact manifests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ForecastManifestValidationError {
    /// Schema version is not the supported value.
    #[error("SP_TX_MANIFEST_SCHEMA_VERSION: schema_version must be 1")]
    InvalidSchemaVersion,
    /// Model identifier is missing.
    #[error("SP_TX_MANIFEST_MODEL_ID_EMPTY: model_id must not be empty")]
    EmptyModelId,
    /// Model identifier exceeds the maximum length.
    #[error("SP_TX_MANIFEST_MODEL_ID_TOO_LONG: model_id exceeds 128 characters")]
    ModelIdTooLong,
    /// Input hash is not a 64-character hex string.
    #[error("SP_TX_MANIFEST_INPUT_HASH_FORMAT: input_hash must be a 64-character hex string")]
    InvalidInputHash,
    /// Output hash is not a 64-character hex string.
    #[error("SP_TX_MANIFEST_OUTPUT_HASH_FORMAT: output_hash must be a 64-character hex string")]
    InvalidOutputHash,
    /// Forecast horizon is outside the accepted range.
    #[error("SP_TX_MANIFEST_HORIZON_RANGE: horizon_hours must be between 1 and 720")]
    InvalidHorizonRange,
    /// Input variable list is empty.
    #[error("SP_TX_MANIFEST_INPUT_VARS_EMPTY: input_variables must not be empty")]
    EmptyInputVariables,
    /// Output variable list is empty.
    #[error("SP_TX_MANIFEST_OUTPUT_VARS_EMPTY: output_variables must not be empty")]
    EmptyOutputVariables,
    /// Input variable list is not sorted.
    #[error("SP_TX_MANIFEST_INPUT_VARS_UNSORTED: input_variables must be sorted")]
    InputVariablesUnsorted,
    /// Output variable list is not sorted.
    #[error("SP_TX_MANIFEST_OUTPUT_VARS_UNSORTED: output_variables must be sorted")]
    OutputVariablesUnsorted,
    /// Input variable list contains duplicates.
    #[error("SP_TX_MANIFEST_INPUT_VARS_DUPLICATES: input_variables contains duplicates")]
    InputVariablesDuplicates,
    /// Output variable list contains duplicates.
    #[error("SP_TX_MANIFEST_OUTPUT_VARS_DUPLICATES: output_variables contains duplicates")]
    OutputVariablesDuplicates,
}

impl ForecastArtifactManifest {
    /// Computes the deterministic digest of the manifest.
    pub fn manifest_digest(&self) -> String {
        payload_digest_hex(self)
    }

    /// Validates manifest fields according to protocol rules.
    pub fn validate(&self) -> Result<(), ForecastManifestValidationError> {
        if self.schema_version != 1 {
            return Err(ForecastManifestValidationError::InvalidSchemaVersion);
        }
        if self.model_id.is_empty() {
            return Err(ForecastManifestValidationError::EmptyModelId);
        }
        if self.model_id.len() > 128 {
            return Err(ForecastManifestValidationError::ModelIdTooLong);
        }
        if !is_valid_hex64(&self.input_hash) {
            return Err(ForecastManifestValidationError::InvalidInputHash);
        }
        if !is_valid_hex64(&self.output_hash) {
            return Err(ForecastManifestValidationError::InvalidOutputHash);
        }
        if !(1..=720).contains(&self.horizon_hours) {
            return Err(ForecastManifestValidationError::InvalidHorizonRange);
        }
        if self.input_variables.is_empty() {
            return Err(ForecastManifestValidationError::EmptyInputVariables);
        }
        if self.output_variables.is_empty() {
            return Err(ForecastManifestValidationError::EmptyOutputVariables);
        }
        if !is_sorted_and_unique(&self.input_variables) {
            return if has_duplicates(&self.input_variables) {
                Err(ForecastManifestValidationError::InputVariablesDuplicates)
            } else {
                Err(ForecastManifestValidationError::InputVariablesUnsorted)
            };
        }
        if !is_sorted_and_unique(&self.output_variables) {
            return if has_duplicates(&self.output_variables) {
                Err(ForecastManifestValidationError::OutputVariablesDuplicates)
            } else {
                Err(ForecastManifestValidationError::OutputVariablesUnsorted)
            };
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ForecastVerificationReceipt
// ---------------------------------------------------------------------------

/// Validation errors for forecast accuracy metrics (blockchain context).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ForecastMetricsValidationError {
    /// MAE must be finite and non-negative.
    #[error("SP_TX_METRICS_MAE_INVALID: mae must be finite and non-negative")]
    InvalidMae,
    /// RMSE must be finite and non-negative.
    #[error("SP_TX_METRICS_RMSE_INVALID: rmse must be finite and non-negative")]
    InvalidRmse,
    /// Bias must be finite.
    #[error("SP_TX_METRICS_BIAS_INVALID: bias must be finite")]
    InvalidBias,
    /// Skill score must be in range.
    #[error("SP_TX_METRICS_SKILL_SCORE_RANGE: skill_score must be in [0, 1]")]
    InvalidSkillScore,
}

/// Validates [`ForecastAccuracyMetrics`] according to blockchain protocol rules.
///
/// This is a richer validation than the simple `&'static str` returned by
/// [`ForecastAccuracyMetrics::validate`], providing typed error variants that
/// can be embedded in [`ForecastReceiptValidationError::InvalidMetrics`].
pub fn validate_metrics(
    metrics: &ForecastAccuracyMetrics,
) -> Result<(), ForecastMetricsValidationError> {
    if !metrics.mae.is_finite() || metrics.mae < 0.0 {
        return Err(ForecastMetricsValidationError::InvalidMae);
    }
    if !metrics.rmse.is_finite() || metrics.rmse < 0.0 {
        return Err(ForecastMetricsValidationError::InvalidRmse);
    }
    if !metrics.bias.is_finite() {
        return Err(ForecastMetricsValidationError::InvalidBias);
    }
    if !metrics.skill_score.is_finite() || !(0.0..=1.0).contains(&metrics.skill_score) {
        return Err(ForecastMetricsValidationError::InvalidSkillScore);
    }
    Ok(())
}

/// Deterministic scoring receipt from forecast verification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForecastVerificationReceipt {
    /// Manifest digest that was verified.
    pub manifest_digest: String,
    /// Forecast commit identifier.
    pub commit_id: String,
    /// Oracle identifier that produced the receipt.
    pub oracle_id: String,
    /// Accuracy metrics recorded by verification.
    pub metrics: ForecastAccuracyMetrics,
    /// Verification timestamp as a Unix epoch in seconds.
    pub verified_at: i64,
    /// Anti-replay nonce.
    pub nonce: String,
    /// Optional hex-encoded ZK proof for model execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zk_proof: Option<String>,
    /// Optional receipt signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// Validation errors for forecast verification receipts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ForecastReceiptValidationError {
    /// Manifest digest is not a 64-character hex string.
    #[error(
        "SP_TX_RECEIPT_MANIFEST_DIGEST_FORMAT: manifest_digest must be a 64-character hex string"
    )]
    InvalidManifestDigest,
    /// Commit identifier is empty.
    #[error("SP_TX_RECEIPT_COMMIT_ID_EMPTY: commit_id must not be empty")]
    EmptyCommitId,
    /// Oracle identifier is empty.
    #[error("SP_TX_RECEIPT_ORACLE_ID_EMPTY: oracle_id must not be empty")]
    EmptyOracleId,
    /// Verification timestamp is invalid.
    #[error("SP_TX_RECEIPT_VERIFIED_AT_INVALID: verified_at must be positive")]
    InvalidVerifiedAt,
    /// Nested metrics payload is invalid.
    #[error("SP_TX_RECEIPT_METRICS_INVALID: {0}")]
    InvalidMetrics(ForecastMetricsValidationError),
}

impl ForecastVerificationReceipt {
    /// Computes the deterministic digest of the receipt.
    pub fn receipt_digest(&self) -> String {
        payload_digest_hex(self)
    }

    fn canonical_signing_message(&self) -> String {
        #[derive(Serialize)]
        struct SigningPayload<'a> {
            manifest_digest: &'a str,
            commit_id: &'a str,
            oracle_id: &'a str,
            metrics: &'a ForecastAccuracyMetrics,
            verified_at: i64,
            nonce: &'a str,
        }

        let digest = payload_digest_hex(&SigningPayload {
            manifest_digest: &self.manifest_digest,
            commit_id: &self.commit_id,
            oracle_id: &self.oracle_id,
            metrics: &self.metrics,
            verified_at: self.verified_at,
            nonce: &self.nonce,
        });
        build_canonical_signing_message(
            DomainSigningAction::ForecastVerification,
            &self.oracle_id,
            self.verified_at,
            Some(&self.nonce),
            &digest,
        )
    }

    /// Builds the canonical signing message for the receipt.
    pub fn signing_message(&self) -> String {
        self.canonical_signing_message()
    }

    /// Verifies the signature against the receipt `oracle_id` public key.
    pub fn has_valid_signature(&self) -> bool {
        let Some(signature_hex) = self.signature.as_deref() else {
            return false;
        };
        let Ok(public_key_bytes) = hex::decode(&self.oracle_id) else {
            return false;
        };
        let Ok(signature_bytes) = hex::decode(signature_hex) else {
            return false;
        };
        let Ok(public_key) = verifying_key_from_bytes(&public_key_bytes) else {
            return false;
        };
        let Ok(signature) = signature_from_bytes(&signature_bytes) else {
            return false;
        };
        public_key
            .verify(self.signing_message().as_bytes(), &signature)
            .is_ok()
    }

    /// Validates receipt fields and nested metrics.
    pub fn validate(&self) -> Result<(), ForecastReceiptValidationError> {
        if !is_valid_hex64(&self.manifest_digest) {
            return Err(ForecastReceiptValidationError::InvalidManifestDigest);
        }
        if self.commit_id.is_empty() {
            return Err(ForecastReceiptValidationError::EmptyCommitId);
        }
        if self.oracle_id.is_empty() {
            return Err(ForecastReceiptValidationError::EmptyOracleId);
        }
        if self.verified_at <= 0 {
            return Err(ForecastReceiptValidationError::InvalidVerifiedAt);
        }
        validate_metrics(&self.metrics)
            .map_err(ForecastReceiptValidationError::InvalidMetrics)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Signing infrastructure
// ---------------------------------------------------------------------------

/// Domain-specific signing action identifiers.
enum DomainSigningAction {
    /// Forecast verification receipt signing.
    ForecastVerification,
}

impl DomainSigningAction {
    /// Returns the domain string embedded in signing messages.
    fn domain(&self) -> &'static str {
        match self {
            Self::ForecastVerification => "wrfcoin.forecast_verification.v1",
        }
    }
}

/// Errors encountered when decoding ed25519 keys or signatures from bytes.
enum SignatureError {
    /// Public key byte slice is not the correct length.
    PublicKeyLength,
    /// Public key bytes do not represent a valid ed25519 key.
    PublicKey,
    /// Signature byte slice is not the correct length.
    SignatureLength,
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Returns `true` if `value` is exactly 64 ASCII hex characters.
fn is_valid_hex64(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

/// Returns `true` if `values` is strictly sorted with no duplicates.
fn is_sorted_and_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

/// Returns `true` if `values` contains any duplicate entries.
fn has_duplicates(values: &[String]) -> bool {
    let mut seen = FxHashSet::default();
    values.iter().any(|value| !seen.insert(value))
}

/// Computes a deterministic SHA3-256 hex digest of the JSON-serialized payload.
fn payload_digest_hex<T: Serialize>(payload: &T) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(serde_json::to_vec(payload).unwrap_or_default());
    hex::encode(hasher.finalize())
}

/// Builds a canonical signing message from the given components.
fn build_canonical_signing_message(
    action: DomainSigningAction,
    signer_id: &str,
    timestamp: i64,
    nonce: Option<&str>,
    payload_digest: &str,
) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        SIGNING_SCHEME_VERSION,
        action.domain(),
        DEFAULT_CHAIN_ID,
        DEFAULT_NETWORK_ID,
        signer_id,
        timestamp,
        nonce.unwrap_or(""),
        payload_digest
    )
}

/// Decodes an ed25519 verifying (public) key from a byte slice.
fn verifying_key_from_bytes(bytes: &[u8]) -> Result<VerifyingKey, SignatureError> {
    let array: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = bytes
        .try_into()
        .map_err(|_| SignatureError::PublicKeyLength)?;
    VerifyingKey::from_bytes(&array).map_err(|_| SignatureError::PublicKey)
}

/// Decodes an ed25519 signature from a byte slice.
fn signature_from_bytes(bytes: &[u8]) -> Result<Ed25519Signature, SignatureError> {
    if bytes.len() != ed25519_dalek::SIGNATURE_LENGTH {
        return Err(SignatureError::SignatureLength);
    }
    let array: [u8; ed25519_dalek::SIGNATURE_LENGTH] = bytes
        .try_into()
        .map_err(|_| SignatureError::SignatureLength)?;
    Ok(Ed25519Signature::from_bytes(&array))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::json;

    fn valid_manifest() -> ForecastArtifactManifest {
        ForecastArtifactManifest {
            schema_version: 1,
            model_id: "wrf-gfs-v3".to_string(),
            input_hash: "a".repeat(64),
            output_hash: "b".repeat(64),
            produced_at: 1_700_000_000,
            region: "us-east".to_string(),
            horizon_hours: 48,
            input_variables: vec!["humidity".to_string(), "pressure".to_string()],
            output_variables: vec!["precipitation".to_string(), "temperature".to_string()],
        }
    }

    fn valid_metrics() -> ForecastAccuracyMetrics {
        ForecastAccuracyMetrics {
            mae: 1.5,
            rmse: 2.0,
            bias: -0.3,
            skill_score: 0.85,
        }
    }

    #[test]
    fn manifest_validation_accepts_valid_input() {
        assert!(valid_manifest().validate().is_ok());
    }

    #[test]
    fn manifest_validation_rejects_bad_schema_version() {
        let mut m = valid_manifest();
        m.schema_version = 2;
        assert!(matches!(
            m.validate(),
            Err(ForecastManifestValidationError::InvalidSchemaVersion)
        ));
    }

    #[test]
    fn manifest_validation_rejects_empty_model_id() {
        let mut m = valid_manifest();
        m.model_id = String::new();
        assert!(matches!(
            m.validate(),
            Err(ForecastManifestValidationError::EmptyModelId)
        ));
    }

    #[test]
    fn manifest_validation_rejects_long_model_id() {
        let mut m = valid_manifest();
        m.model_id = "x".repeat(129);
        assert!(matches!(
            m.validate(),
            Err(ForecastManifestValidationError::ModelIdTooLong)
        ));
    }

    #[test]
    fn manifest_validation_rejects_bad_input_hash() {
        let mut m = valid_manifest();
        m.input_hash = "not-hex".to_string();
        assert!(matches!(
            m.validate(),
            Err(ForecastManifestValidationError::InvalidInputHash)
        ));
    }

    #[test]
    fn manifest_validation_rejects_bad_output_hash() {
        let mut m = valid_manifest();
        m.output_hash = "short".to_string();
        assert!(matches!(
            m.validate(),
            Err(ForecastManifestValidationError::InvalidOutputHash)
        ));
    }

    #[test]
    fn manifest_validation_rejects_bad_horizon() {
        let mut m = valid_manifest();
        m.horizon_hours = 0;
        assert!(matches!(
            m.validate(),
            Err(ForecastManifestValidationError::InvalidHorizonRange)
        ));
        m.horizon_hours = 721;
        assert!(matches!(
            m.validate(),
            Err(ForecastManifestValidationError::InvalidHorizonRange)
        ));
    }

    #[test]
    fn manifest_validation_rejects_empty_vars() {
        let mut m = valid_manifest();
        m.input_variables = vec![];
        assert!(matches!(
            m.validate(),
            Err(ForecastManifestValidationError::EmptyInputVariables)
        ));

        let mut m2 = valid_manifest();
        m2.output_variables = vec![];
        assert!(matches!(
            m2.validate(),
            Err(ForecastManifestValidationError::EmptyOutputVariables)
        ));
    }

    #[test]
    fn manifest_validation_rejects_unsorted_vars() {
        let mut m = valid_manifest();
        m.input_variables = vec!["z".to_string(), "a".to_string()];
        assert!(matches!(
            m.validate(),
            Err(ForecastManifestValidationError::InputVariablesUnsorted)
        ));
    }

    #[test]
    fn manifest_validation_rejects_duplicate_vars() {
        let mut m = valid_manifest();
        m.input_variables = vec!["a".to_string(), "a".to_string()];
        assert!(matches!(
            m.validate(),
            Err(ForecastManifestValidationError::InputVariablesDuplicates)
        ));
    }

    #[test]
    fn manifest_digest_is_hex64() {
        let manifest = valid_manifest();
        let digest = manifest.manifest_digest();
        assert_eq!(digest.len(), 64);
        assert!(digest.chars().all(|ch| ch.is_ascii_hexdigit()));
    }

    #[test]
    fn metrics_validation_accepts_valid() {
        assert!(validate_metrics(&valid_metrics()).is_ok());
    }

    #[test]
    fn metrics_validation_rejects_invalid_mae() {
        let mut m = valid_metrics();
        m.mae = -1.0;
        assert!(matches!(
            validate_metrics(&m),
            Err(ForecastMetricsValidationError::InvalidMae)
        ));
    }

    #[test]
    fn metrics_validation_rejects_invalid_rmse() {
        let mut m = valid_metrics();
        m.rmse = f64::NAN;
        assert!(matches!(
            validate_metrics(&m),
            Err(ForecastMetricsValidationError::InvalidRmse)
        ));
    }

    #[test]
    fn metrics_validation_rejects_invalid_bias() {
        let mut m = valid_metrics();
        m.bias = f64::INFINITY;
        assert!(matches!(
            validate_metrics(&m),
            Err(ForecastMetricsValidationError::InvalidBias)
        ));
    }

    #[test]
    fn metrics_validation_rejects_invalid_skill_score() {
        let mut m = valid_metrics();
        m.skill_score = 1.5;
        assert!(matches!(
            validate_metrics(&m),
            Err(ForecastMetricsValidationError::InvalidSkillScore)
        ));
    }

    #[test]
    fn receipt_validation_rejects_invalid_metrics() {
        let mut receipt = ForecastVerificationReceipt {
            manifest_digest: "a".repeat(64),
            commit_id: "commit-1".to_string(),
            oracle_id: "oracle-1".to_string(),
            metrics: valid_metrics(),
            verified_at: 1_700_000_100,
            nonce: "nonce-1".to_string(),
            zk_proof: None,
            signature: None,
        };
        receipt.metrics.skill_score = 1.5;

        assert!(matches!(
            receipt.validate(),
            Err(ForecastReceiptValidationError::InvalidMetrics(
                ForecastMetricsValidationError::InvalidSkillScore
            ))
        ));
    }

    #[test]
    fn receipt_validation_rejects_bad_manifest_digest() {
        let receipt = ForecastVerificationReceipt {
            manifest_digest: "too-short".to_string(),
            commit_id: "commit-1".to_string(),
            oracle_id: "oracle-1".to_string(),
            metrics: valid_metrics(),
            verified_at: 1_700_000_100,
            nonce: "nonce-1".to_string(),
            zk_proof: None,
            signature: None,
        };
        assert!(matches!(
            receipt.validate(),
            Err(ForecastReceiptValidationError::InvalidManifestDigest)
        ));
    }

    #[test]
    fn receipt_validation_rejects_empty_commit_id() {
        let receipt = ForecastVerificationReceipt {
            manifest_digest: "a".repeat(64),
            commit_id: String::new(),
            oracle_id: "oracle-1".to_string(),
            metrics: valid_metrics(),
            verified_at: 1_700_000_100,
            nonce: "nonce-1".to_string(),
            zk_proof: None,
            signature: None,
        };
        assert!(matches!(
            receipt.validate(),
            Err(ForecastReceiptValidationError::EmptyCommitId)
        ));
    }

    #[test]
    fn receipt_validation_rejects_empty_oracle_id() {
        let receipt = ForecastVerificationReceipt {
            manifest_digest: "a".repeat(64),
            commit_id: "commit-1".to_string(),
            oracle_id: String::new(),
            metrics: valid_metrics(),
            verified_at: 1_700_000_100,
            nonce: "nonce-1".to_string(),
            zk_proof: None,
            signature: None,
        };
        assert!(matches!(
            receipt.validate(),
            Err(ForecastReceiptValidationError::EmptyOracleId)
        ));
    }

    #[test]
    fn receipt_validation_rejects_invalid_verified_at() {
        let receipt = ForecastVerificationReceipt {
            manifest_digest: "a".repeat(64),
            commit_id: "commit-1".to_string(),
            oracle_id: "oracle-1".to_string(),
            metrics: valid_metrics(),
            verified_at: 0,
            nonce: "nonce-1".to_string(),
            zk_proof: None,
            signature: None,
        };
        assert!(matches!(
            receipt.validate(),
            Err(ForecastReceiptValidationError::InvalidVerifiedAt)
        ));
    }

    #[test]
    fn receipt_deserializes_when_optional_fields_are_missing() {
        let receipt: ForecastVerificationReceipt = serde_json::from_value(json!({
            "manifest_digest": "a".repeat(64),
            "commit_id": "commit-1",
            "oracle_id": "oracle-1",
            "metrics": {
                "mae": 1.5,
                "rmse": 2.0,
                "bias": -0.3,
                "skill_score": 0.85
            },
            "verified_at": 1_700_000_100i64,
            "nonce": "nonce-1"
        }))
        .expect("missing optional fields should deserialize with defaults");

        assert_eq!(receipt.zk_proof, None);
        assert_eq!(receipt.signature, None);
    }

    #[test]
    fn receipt_digest_is_hex64() {
        let receipt = ForecastVerificationReceipt {
            manifest_digest: "a".repeat(64),
            commit_id: "commit-1".to_string(),
            oracle_id: "oracle-1".to_string(),
            metrics: valid_metrics(),
            verified_at: 1_700_000_100,
            nonce: "nonce-1".to_string(),
            zk_proof: None,
            signature: None,
        };
        let digest = receipt.receipt_digest();
        assert_eq!(digest.len(), 64);
        assert!(digest.chars().all(|ch| ch.is_ascii_hexdigit()));
    }

    #[test]
    fn receipt_signature_round_trip() {
        let signing_key = SigningKey::from_bytes(&[7u8; ed25519_dalek::SECRET_KEY_LENGTH]);
        let verifying_key = signing_key.verifying_key();

        let mut receipt = ForecastVerificationReceipt {
            manifest_digest: "a".repeat(64),
            commit_id: "commit-1".to_string(),
            oracle_id: hex::encode(verifying_key.to_bytes()),
            metrics: valid_metrics(),
            verified_at: 1_700_000_100,
            nonce: "nonce-1".to_string(),
            zk_proof: None,
            signature: None,
        };

        let signature = signing_key.sign(receipt.signing_message().as_bytes());
        receipt.signature = Some(hex::encode(signature.to_bytes()));

        assert_eq!(receipt.receipt_digest().len(), 64);
        assert!(receipt.has_valid_signature());
    }

    #[test]
    fn has_valid_signature_returns_false_without_signature() {
        let receipt = ForecastVerificationReceipt {
            manifest_digest: "a".repeat(64),
            commit_id: "commit-1".to_string(),
            oracle_id: "oracle-1".to_string(),
            metrics: valid_metrics(),
            verified_at: 1_700_000_100,
            nonce: "nonce-1".to_string(),
            zk_proof: None,
            signature: None,
        };
        assert!(!receipt.has_valid_signature());
    }

    #[test]
    fn has_valid_signature_returns_false_for_bad_oracle_id() {
        let receipt = ForecastVerificationReceipt {
            manifest_digest: "a".repeat(64),
            commit_id: "commit-1".to_string(),
            oracle_id: "not-hex".to_string(),
            metrics: valid_metrics(),
            verified_at: 1_700_000_100,
            nonce: "nonce-1".to_string(),
            zk_proof: None,
            signature: Some("ab".repeat(64)),
        };
        assert!(!receipt.has_valid_signature());
    }
}
