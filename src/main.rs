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
        extract::State,
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
            .fallback_service(ServeDir::new("./dist"))
            .layer(cors)
            .with_state(state);

        let address = SocketAddr::from(([0, 0, 0, 0], 3000));
        let listener = tokio::net::TcpListener::bind(address).await?;
        println!("Cadmus gateway listening on http://{address}");
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
