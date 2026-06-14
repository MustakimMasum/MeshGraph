#[cfg(target_arch = "wasm32")]
mod app;

#[cfg(target_arch = "wasm32")]
fn main() {
    leptos::mount_to_body(app::App);
}

#[cfg(not(target_arch = "wasm32"))]
mod gateway {
    use std::{env, net::SocketAddr};

    use axum::{
        extract::{Path, State},
        http::{header, StatusCode},
        routing::get,
        Json, Router,
    };
    use reqwest::Client;
    use serde::{Deserialize, Serialize};
    use tower_http::{
        cors::{Any, CorsLayer},
        services::ServeDir,
    };

    const DEFAULT_DATABASE_URL: &str = "http://localhost:7878/query";
    const STRUCTURE_QUERY: &str = r#"
PREFIX ex: <http://example.org/cadmus/>
SELECT ?partName ?x ?y ?z WHERE {
    ex:SojournerRover ex:hasPart ?part .
    ?part ex:offsetX ?x ; ex:offsetY ?y ; ex:offsetZ ?z .
    BIND(STRAFTER(STR(?part), "http://example.org/cadmus/") AS ?partName)
}
ORDER BY ?partName
"#;

    #[derive(Clone)]
    struct AppState {
        database_url: String,
        client: Client,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct SparqlEnvelope {
        results: SparqlResults,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct SparqlResults {
        bindings: Vec<StructureBinding>,
    }

    #[derive(Debug, Deserialize)]
    struct MetadataEnvelope {
        results: MetadataResults,
    }

    #[derive(Debug, Deserialize)]
    struct MetadataResults {
        bindings: Vec<MetadataBinding>,
    }

    #[derive(Debug, Deserialize)]
    struct RelatedComponentsEnvelope {
        results: RelatedComponentsResults,
    }

    #[derive(Debug, Deserialize)]
    struct RelatedComponentsResults {
        bindings: Vec<RelatedComponentBinding>,
    }

    #[derive(Debug, Deserialize)]
    struct RelatedComponentBinding {
        #[serde(rename = "relatedPart")]
        related_part: SparqlValue,
    }

    #[derive(Debug, Deserialize)]
    struct MetadataBinding {
        #[serde(rename = "displayName")]
        display_name: Option<SparqlValue>,
        category: Option<SparqlValue>,
        purpose: Option<SparqlValue>,
        #[serde(rename = "powerRequirement")]
        power_requirement: Option<SparqlValue>,
        #[serde(rename = "missionNote")]
        mission_note: Option<SparqlValue>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct StructureBinding {
        #[serde(rename = "partName")]
        part_name: SparqlValue,
        x: SparqlValue,
        y: SparqlValue,
        z: SparqlValue,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct SparqlValue {
        #[serde(rename = "type")]
        value_type: String,
        value: String,
        datatype: Option<String>,
    }

    #[derive(Debug, Serialize)]
    struct ApiError {
        error: String,
    }

    #[derive(Debug, Serialize)]
    struct ComponentMetadata {
        component_name: String,
        display_name: String,
        category: Option<String>,
        purpose: Option<String>,
        power_requirement: Option<String>,
        mission_note: Option<String>,
    }

    impl ApiError {
        fn new(error: impl Into<String>) -> Self {
            Self {
                error: error.into(),
            }
        }
    }

    type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiError>)>;

    pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
        let state = AppState {
            database_url: database_url(env::var("DATABASE_URL").ok().as_deref()),
            client: Client::new(),
        };

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = Router::new()
            .route("/api/v1/health", get(health))
            .route("/api/v1/structure", get(structure))
            .route(
                "/api/v1/components/:component_name",
                get(component_metadata),
            )
            .route(
                "/api/v1/components/:component_name/related",
                get(related_components),
            )
            .fallback_service(ServeDir::new("./dist"))
            .layer(cors)
            .with_state(state);

        let address = SocketAddr::from(([0, 0, 0, 0], 3000));
        let listener = tokio::net::TcpListener::bind(address).await?;
        println!("Cadmus gateway listening on http://localhost:3000");
        axum::serve(listener, app).await?;
        Ok(())
    }

    async fn health() -> StatusCode {
        StatusCode::NO_CONTENT
    }

    async fn structure(State(state): State<AppState>) -> ApiResult<Vec<StructureBinding>> {
        let response = state
            .client
            .post(&state.database_url)
            .header(header::CONTENT_TYPE, "application/sparql-query")
            .header(header::ACCEPT, "application/sparql-results+json")
            .body(STRUCTURE_QUERY)
            .send()
            .await
            .map_err(|error| upstream_error(format!("Oxigraph request failed: {error}")))?
            .error_for_status()
            .map_err(|error| upstream_error(format!("Oxigraph returned an error: {error}")))?;

        let envelope = response
            .json::<SparqlEnvelope>()
            .await
            .map_err(|error| upstream_error(format!("Invalid Oxigraph response: {error}")))?;

        Ok(Json(envelope.results.bindings))
    }

    async fn component_metadata(
        State(state): State<AppState>,
        Path(component_name): Path<String>,
    ) -> ApiResult<ComponentMetadata> {
        let query = metadata_query(&component_name).ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("Invalid component name")),
            )
        })?;

        let response = state
            .client
            .post(&state.database_url)
            .header(header::CONTENT_TYPE, "application/sparql-query")
            .header(header::ACCEPT, "application/sparql-results+json")
            .body(query)
            .send()
            .await
            .map_err(|error| upstream_error(format!("Oxigraph request failed: {error}")))?
            .error_for_status()
            .map_err(|error| upstream_error(format!("Oxigraph returned an error: {error}")))?;

        let envelope = response
            .json::<MetadataEnvelope>()
            .await
            .map_err(|error| upstream_error(format!("Invalid Oxigraph response: {error}")))?;
        let binding = envelope
            .results
            .bindings
            .into_iter()
            .next()
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    Json(ApiError::new("Component metadata not found")),
                )
            })?;

        Ok(Json(ComponentMetadata {
            display_name: binding
                .display_name
                .map(|value| value.value)
                .unwrap_or_else(|| component_name.clone()),
            category: binding.category.map(|value| value.value),
            purpose: binding.purpose.map(|value| value.value),
            power_requirement: binding.power_requirement.map(|value| value.value),
            mission_note: binding.mission_note.map(|value| value.value),
            component_name,
        }))
    }

    async fn related_components(
        State(state): State<AppState>,
        Path(component_name): Path<String>,
    ) -> ApiResult<Vec<String>> {
        if component_name.is_empty()
            || !component_name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("Invalid component name")),
            ));
        }

        let query = format!(
            r#"
PREFIX ex: <http://example.org/cadmus/>
SELECT DISTINCT ?relatedPart WHERE {{
    BIND(ex:{component_name} AS ?part)
    {{ ?part ex:connectedTo ?related }} UNION {{ ?related ex:connectedTo ?part }}
    BIND(STRAFTER(STR(?related), "http://example.org/cadmus/") AS ?relatedPart)
}}
"#
        );

        let response = state
            .client
            .post(&state.database_url)
            .header(header::CONTENT_TYPE, "application/sparql-query")
            .header(header::ACCEPT, "application/sparql-results+json")
            .body(query)
            .send()
            .await
            .map_err(|error| upstream_error(format!("Oxigraph request failed: {error}")))?
            .error_for_status()
            .map_err(|error| upstream_error(format!("Oxigraph returned an error: {error}")))?;

        let envelope = response
            .json::<RelatedComponentsEnvelope>()
            .await
            .map_err(|error| upstream_error(format!("Invalid Oxigraph response: {error}")))?;

        let related: Vec<String> = envelope
            .results
            .bindings
            .into_iter()
            .map(|binding| binding.related_part.value)
            .collect();

        Ok(Json(related))
    }

    fn metadata_query(component_name: &str) -> Option<String> {
        if component_name.is_empty()
            || !component_name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return None;
        }

        Some(format!(
            r#"
PREFIX ex: <http://example.org/cadmus/>
SELECT ?displayName ?category ?purpose ?powerRequirement ?missionNote WHERE {{
    BIND(ex:{component_name} AS ?part)
    OPTIONAL {{ ?part ex:displayName ?displayName . }}
    OPTIONAL {{ ?part ex:category ?category . }}
    OPTIONAL {{ ?part ex:purpose ?purpose . }}
    OPTIONAL {{ ?part ex:powerRequirement ?powerRequirement . }}
    OPTIONAL {{ ?part ex:missionNote ?missionNote . }}
}}
"#
        ))
    }

    fn database_url(value: Option<&str>) -> String {
        value
            .filter(|url| !url.trim().is_empty())
            .unwrap_or(DEFAULT_DATABASE_URL)
            .to_owned()
    }

    fn upstream_error(message: String) -> (StatusCode, Json<ApiError>) {
        (StatusCode::BAD_GATEWAY, Json(ApiError::new(message)))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn database_url_uses_default_for_missing_or_empty_values() {
            assert_eq!(database_url(None), DEFAULT_DATABASE_URL);
            assert_eq!(database_url(Some("  ")), DEFAULT_DATABASE_URL);
        }

        #[test]
        fn parses_oxigraph_binding_array() {
            let response = r#"{
                "results": {
                    "bindings": [{
                        "partName": {"type": "literal", "value": "ChassisBase"},
                        "x": {"type": "literal", "value": "0.0"},
                        "y": {"type": "literal", "value": "0.0"},
                        "z": {"type": "literal", "value": "0.0"}
                    }]
                }
            }"#;

            let parsed: SparqlEnvelope = serde_json::from_str(response).expect("valid response");
            assert_eq!(parsed.results.bindings[0].part_name.value, "ChassisBase");
        }

        #[test]
        fn metadata_query_rejects_non_identifier_component_names() {
            assert!(metadata_query("Antenna").is_some());
            assert!(metadata_query("WheelFrontLeft").is_some());
            assert!(metadata_query("../query").is_none());
            assert!(metadata_query("Antenna> ?s ?p ?o").is_none());
        }

        #[test]
        fn parses_optional_component_metadata() {
            let response = r#"{
                "results": {
                    "bindings": [{
                        "displayName": {"type": "literal", "value": "Low-Gain Antenna"},
                        "purpose": {"type": "literal", "value": "Returns telemetry"}
                    }]
                }
            }"#;

            let parsed: MetadataEnvelope = serde_json::from_str(response).expect("valid response");
            let binding = &parsed.results.bindings[0];
            assert_eq!(
                binding
                    .display_name
                    .as_ref()
                    .map(|value| value.value.as_str()),
                Some("Low-Gain Antenna")
            );
            assert!(binding.power_requirement.is_none());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() {
    if let Err(error) = gateway::run().await {
        eprintln!("Cadmus gateway failed: {error}");
        std::process::exit(1);
    }
}
