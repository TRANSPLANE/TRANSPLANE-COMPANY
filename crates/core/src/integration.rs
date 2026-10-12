//! Host-neutral boundaries for a future optional world adapter.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubjectReference {
    pub issuer: String,
    pub subject: String,
    pub kind: SubjectKind,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    Human,
    Agent,
    Service,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceReference {
    pub service: String,
    pub company_id: String,
    pub kind: String,
    pub id: String,
    pub version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedProjection {
    pub version: u32,
    pub company_id: String,
    pub selected_fields: Value,
    pub authorized_by: SubjectReference,
}
/// Implementations belong outside the company core. They receive only selected projections.
pub trait WorldAdapter {
    type Error;
    fn publish(
        &self,
        projection: &SharedProjection,
        idempotency_key: &str,
    ) -> std::result::Result<String, Self::Error>;
    fn revoke(
        &self,
        external_reference: &str,
        idempotency_key: &str,
    ) -> std::result::Result<(), Self::Error>;
}
