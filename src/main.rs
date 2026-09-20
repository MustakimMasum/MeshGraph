#[cfg(target_arch = "wasm32")]
mod app;

#[cfg(target_arch = "wasm32")]
fn main() {
    leptos::mount_to_body(app::App);
}

#[cfg(not(target_arch = "wasm32"))]
mod ingestion;

#[cfg(not(target_arch = "wasm32"))]
mod gateway {
    use std::{env, net::SocketAddr};

    use axum::{
        extract::{Path, Query, State},
        http::{header, HeaderValue, StatusCode},
        routing::{get, post},
        Json, Router,
    };
    use reqwest::Client;
    use serde::{de::DeserializeOwned, Deserialize, Serialize};
    use tower_http::{
        cors::{Any, CorsLayer},
        services::ServeDir,
        set_header::SetResponseHeaderLayer,
    };

    use crate::ingestion::{ingest_openalex_network, IngestionError, IngestionSummary};

    const DEFAULT_DATABASE_URL: &str = "http://localhost:7878/query";
    const CITATION_GRAPH: &str = "http://example.org/meshgraph/graphs/citations";

    const NODE_QUERY: &str = r#"
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
PREFIX schema: <https://schema.org/>
PREFIX dcterms: <http://purl.org/dc/terms/>
PREFIX ex: <http://example.org/meshgraph/>
SELECT ?paper (SAMPLE(?paperTitle) AS ?title) (SAMPLE(?created) AS ?year)
       (SAMPLE(?paperTopic) AS ?topic) (SAMPLE(?count) AS ?citationCount)
WHERE {
  GRAPH <http://example.org/meshgraph/graphs/citations> {
    ?paper rdf:type schema:ScholarlyArticle .
    OPTIONAL { ?paper schema:name ?paperTitle }
    OPTIONAL { ?paper dcterms:created ?created }
    OPTIONAL { ?paper schema:about ?paperTopic }
    OPTIONAL { ?paper ex:citationCount ?count }
  }
}
GROUP BY ?paper
ORDER BY ?year ?paper
"#;

    const DIRECT_LINK_QUERY: &str = r#"
PREFIX cito: <http://purl.org/spar/cito/>
SELECT ?source ?target (1 AS ?weight) WHERE {
  GRAPH <http://example.org/meshgraph/graphs/citations> { ?source cito:cites ?target }
}
"#;

    const CO_CITATION_QUERY: &str = r#"
PREFIX cito: <http://purl.org/spar/cito/>
SELECT ?source ?target (COUNT(DISTINCT ?citingPaper) AS ?weight) WHERE {
  GRAPH <http://example.org/meshgraph/graphs/citations> {
    ?citingPaper cito:cites ?source, ?target .
    FILTER(STR(?source) < STR(?target))
  }
}
GROUP BY ?source ?target
"#;

    const BIBLIOGRAPHIC_COUPLING_QUERY: &str = r#"
PREFIX cito: <http://purl.org/spar/cito/>
SELECT ?source ?target (COUNT(DISTINCT ?reference) AS ?weight) WHERE {
  GRAPH <http://example.org/meshgraph/graphs/citations> {
    ?source cito:cites ?reference .
    ?target cito:cites ?reference .
    FILTER(STR(?source) < STR(?target))
  }
}
GROUP BY ?source ?target
"#;

    #[derive(Clone)]
    struct AppState {
        query_url: String,
        update_url: String,
        openalex_email: Option<String>,
        client: Client,
    }

    #[derive(Debug, Deserialize)]
    struct SparqlEnvelope<T> {
        results: SparqlResults<T>,
    }

    #[derive(Debug, Deserialize)]
    struct SparqlResults<T> {
        bindings: Vec<T>,
    }

    #[derive(Clone, Debug, Deserialize)]
    struct SparqlValue {
        value: String,
    }

    #[derive(Debug, Deserialize)]
    struct NodeBinding {
        paper: SparqlValue,
        title: Option<SparqlValue>,
        year: Option<SparqlValue>,
        topic: Option<SparqlValue>,
        #[serde(rename = "citationCount")]
        citation_count: Option<SparqlValue>,
    }

    #[derive(Debug, Deserialize)]
    struct LinkBinding {
        source: SparqlValue,
        target: SparqlValue,
        weight: Option<SparqlValue>,
    }

    #[derive(Debug, Deserialize)]
    struct PaperBinding {
        paper: SparqlValue,
        title: Option<SparqlValue>,
        year: Option<SparqlValue>,
        topic: Option<SparqlValue>,
        genre: Option<SparqlValue>,
        abstract_text: Option<SparqlValue>,
        doi: Option<SparqlValue>,
        pdf_url: Option<SparqlValue>,
        citation_count: Option<SparqlValue>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct GraphNode {
        id: String,
        title: String,
        year: Option<i32>,
        topic: String,
        citation_count: u64,
    }

    #[derive(Debug, Serialize)]
    struct GraphLink {
        source: String,
        target: String,
        kind: &'static str,
        weight: u64,
    }

    #[derive(Debug, Serialize)]
    struct CitationGraph {
        nodes: Vec<GraphNode>,
        links: Vec<GraphLink>,
        mode: &'static str,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct PaperMetadata {
        id: String,
        title: String,
        year: Option<i32>,
        topic: Option<String>,
        genre: Option<String>,
        abstract_text: Option<String>,
        doi: Option<String>,
        pdf_url: Option<String>,
        citation_count: u64,
    }

    #[derive(Debug, Deserialize)]
    struct IngestRequest {
        #[serde(alias = "identifier")]
        doi: String,
        #[serde(default = "default_depth")]
        depth: u8,
    }

    #[derive(Debug, Default, Deserialize)]
    struct GraphOptions {
        mode: Option<String>,
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
        let query_url = database_url(env::var("DATABASE_URL").ok().as_deref());
        let state = AppState {
            update_url: update_url(&query_url, env::var("DATABASE_UPDATE_URL").ok().as_deref()),
            query_url,
            openalex_email: env::var("OPENALEX_EMAIL").ok(),
            client: Client::builder().user_agent("MeshGraph/0.2").build()?,
        };

        let app = Router::new()
            .route("/api/v1/health", get(health))
            .route("/api/v1/citations/ingest", post(ingest))
            .route("/api/v1/citations/graph", get(citation_graph))
            .route("/api/v1/citations/paper/:id", get(paper_metadata))
            .fallback_service(ServeDir::new("./dist").append_index_html_on_directories(true))
            .layer(SetResponseHeaderLayer::if_not_present(
                header::HeaderName::from_static("cross-origin-opener-policy"),
                HeaderValue::from_static("same-origin"),
            ))
            .layer(SetResponseHeaderLayer::if_not_present(
                header::HeaderName::from_static("cross-origin-embedder-policy"),
                HeaderValue::from_static("credentialless"),
            ))
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .with_state(state);

        let address = SocketAddr::from(([0, 0, 0, 0], 3000));
        let listener = tokio::net::TcpListener::bind(address).await?;
        println!("MeshGraph gateway listening on http://localhost:3000");
        axum::serve(listener, app).await?;
        Ok(())
    }

    async fn health() -> StatusCode {
        StatusCode::NO_CONTENT
    }

    async fn ingest(
        State(state): State<AppState>,
        Json(request): Json<IngestRequest>,
    ) -> ApiResult<IngestionSummary> {
        ingest_openalex_network(
            &state.client,
            &state.update_url,
            &request.doi,
            request.depth,
            state.openalex_email.as_deref(),
        )
        .await
        .map(Json)
        .map_err(|error| match error {
            IngestionError::InvalidIdentifier(message) => {
                (StatusCode::BAD_REQUEST, Json(ApiError::new(message)))
            }
            IngestionError::OpenAlex(message) | IngestionError::Rdf(message) => {
                (StatusCode::BAD_GATEWAY, Json(ApiError::new(message)))
            }
        })
    }

    async fn citation_graph(
        State(state): State<AppState>,
        Query(options): Query<GraphOptions>,
    ) -> ApiResult<CitationGraph> {
        let (mode, link_query) = topology_query(options.mode.as_deref())?;
        let (node_envelope, link_envelope) = tokio::try_join!(
            run_query::<NodeBinding>(&state, NODE_QUERY),
            run_query::<LinkBinding>(&state, link_query)
        )?;

        let nodes = node_envelope
            .results
            .bindings
            .into_iter()
            .map(|binding| GraphNode {
                id: compact_id(&binding.paper.value),
                title: binding
                    .title
                    .map(|value| value.value)
                    .unwrap_or_else(|| "Untitled paper".to_owned()),
                year: binding.year.and_then(|value| value.value.parse().ok()),
                topic: binding
                    .topic
                    .map(|value| value.value)
                    .unwrap_or_else(|| "Unclassified".to_owned()),
                citation_count: binding
                    .citation_count
                    .and_then(|value| value.value.parse().ok())
                    .unwrap_or(0),
            })
            .collect();

        let links = link_envelope
            .results
            .bindings
            .into_iter()
            .map(|binding| GraphLink {
                source: compact_id(&binding.source.value),
                target: compact_id(&binding.target.value),
                kind: mode,
                weight: binding
                    .weight
                    .and_then(|value| value.value.parse().ok())
                    .unwrap_or(1),
            })
            .collect();

        Ok(Json(CitationGraph { nodes, links, mode }))
    }

    async fn paper_metadata(
        State(state): State<AppState>,
        Path(id): Path<String>,
    ) -> ApiResult<PaperMetadata> {
        let paper_iri = paper_iri(&id).ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiError::new("Paper id must be an OpenAlex W identifier")),
            )
        })?;
        let query = format!(
            r#"
PREFIX schema: <https://schema.org/>
PREFIX dcterms: <http://purl.org/dc/terms/>
PREFIX ex: <http://example.org/meshgraph/>
SELECT ?paper (SAMPLE(?paperTitle) AS ?title) (SAMPLE(?created) AS ?year)
       (SAMPLE(?paperTopic) AS ?topic) (SAMPLE(?paperGenre) AS ?genre)
       (SAMPLE(?paperAbstract) AS ?abstract_text) (SAMPLE(?identifier) AS ?doi)
       (SAMPLE(?url) AS ?pdf_url) (SAMPLE(?count) AS ?citation_count)
WHERE {{
  GRAPH <{CITATION_GRAPH}> {{
    BIND(<{paper_iri}> AS ?paper)
    ?paper a schema:ScholarlyArticle .
    OPTIONAL {{ ?paper schema:name ?paperTitle }}
    OPTIONAL {{ ?paper dcterms:created ?created }}
    OPTIONAL {{ ?paper schema:about ?paperTopic }}
    OPTIONAL {{ ?paper schema:genre ?paperGenre }}
    OPTIONAL {{ ?paper schema:abstract ?paperAbstract }}
    OPTIONAL {{ ?paper schema:identifier ?identifier }}
    OPTIONAL {{ ?paper schema:url ?url }}
    OPTIONAL {{ ?paper ex:citationCount ?count }}
  }}
}}
GROUP BY ?paper
"#
        );
        let envelope = run_query::<PaperBinding>(&state, &query).await?;
        let binding = envelope
            .results
            .bindings
            .into_iter()
            .next()
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    Json(ApiError::new("Paper not found")),
                )
            })?;

        Ok(Json(PaperMetadata {
            id: compact_id(&binding.paper.value),
            title: binding
                .title
                .map(|value| value.value)
                .unwrap_or_else(|| "Untitled paper".to_owned()),
            year: binding.year.and_then(|value| value.value.parse().ok()),
            topic: binding.topic.map(|value| value.value),
            genre: binding.genre.map(|value| value.value),
            abstract_text: binding.abstract_text.map(|value| value.value),
            doi: binding.doi.map(|value| value.value),
            pdf_url: binding.pdf_url.map(|value| value.value),
            citation_count: binding
                .citation_count
                .and_then(|value| value.value.parse().ok())
                .unwrap_or(0),
        }))
    }

    async fn run_query<T>(
        state: &AppState,
        query: &str,
    ) -> Result<SparqlEnvelope<T>, (StatusCode, Json<ApiError>)>
    where
        T: DeserializeOwned,
    {
        state
            .client
            .post(&state.query_url)
            .header(header::CONTENT_TYPE, "application/sparql-query")
            .header(header::ACCEPT, "application/sparql-results+json")
            .body(query.to_owned())
            .send()
            .await
            .map_err(|error| upstream_error(format!("Oxigraph request failed: {error}")))?
            .error_for_status()
            .map_err(|error| upstream_error(format!("Oxigraph returned an error: {error}")))?
            .json::<SparqlEnvelope<T>>()
            .await
            .map_err(|error| upstream_error(format!("Invalid Oxigraph response: {error}")))
    }

    fn topology_query(
        mode: Option<&str>,
    ) -> Result<(&'static str, &'static str), (StatusCode, Json<ApiError>)> {
        match mode.unwrap_or("citations") {
            "citations" => Ok(("citations", DIRECT_LINK_QUERY)),
            "co-citation" => Ok(("co-citation", CO_CITATION_QUERY)),
            "bibliographic-coupling" => {
                Ok(("bibliographic-coupling", BIBLIOGRAPHIC_COUPLING_QUERY))
            }
            _ => Err((
                StatusCode::BAD_REQUEST,
                Json(ApiError::new(
                    "mode must be citations, co-citation, or bibliographic-coupling",
                )),
            )),
        }
    }

    fn default_depth() -> u8 {
        1
    }

    fn database_url(value: Option<&str>) -> String {
        value
            .filter(|url| !url.trim().is_empty())
            .unwrap_or(DEFAULT_DATABASE_URL)
            .to_owned()
    }

    fn update_url(query_url: &str, explicit: Option<&str>) -> String {
        explicit
            .filter(|url| !url.trim().is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                query_url
                    .strip_suffix("/query")
                    .map(|base| format!("{base}/update"))
                    .unwrap_or_else(|| format!("{query_url}/../update"))
            })
    }

    fn paper_iri(id: &str) -> Option<String> {
        let id = id.trim();
        (id.len() > 1
            && id.starts_with('W')
            && id[1..].chars().all(|character| character.is_ascii_digit()))
        .then(|| format!("https://openalex.org/{id}"))
    }

    fn compact_id(iri: &str) -> String {
        iri.rsplit('/').next().unwrap_or(iri).to_owned()
    }

    fn upstream_error(message: String) -> (StatusCode, Json<ApiError>) {
        (StatusCode::BAD_GATEWAY, Json(ApiError::new(message)))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn database_urls_have_safe_fallbacks() {
            assert_eq!(database_url(None), DEFAULT_DATABASE_URL);
            assert_eq!(database_url(Some("  ")), DEFAULT_DATABASE_URL);
            assert_eq!(
                update_url(DEFAULT_DATABASE_URL, None),
                "http://localhost:7878/update"
            );
        }

        #[test]
        fn paper_identifier_validation_blocks_sparql_injection() {
            assert_eq!(
                paper_iri("W123"),
                Some("https://openalex.org/W123".to_owned())
            );
            assert!(paper_iri("W1> ?s ?p ?o").is_none());
            assert!(paper_iri("https://openalex.org/W1").is_none());
        }

        #[test]
        fn topology_modes_select_distinct_sparql_constructs() {
            assert!(topology_query(Some("citations")).is_ok());
            assert!(topology_query(Some("co-citation")).is_ok());
            assert!(topology_query(Some("bibliographic-coupling")).is_ok());
            assert!(topology_query(Some("unknown")).is_err());
        }

        #[test]
        fn parses_optional_sparql_node_values() {
            let response = r#"{
                "results": {"bindings": [{
                    "paper": {"type": "uri", "value": "https://openalex.org/W1"},
                    "title": {"type": "literal", "value": "A paper"}
                }]}
            }"#;
            let parsed: SparqlEnvelope<NodeBinding> =
                serde_json::from_str(response).expect("valid SPARQL response");
            assert_eq!(
                parsed.results.bindings[0].paper.value,
                "https://openalex.org/W1"
            );
            assert!(parsed.results.bindings[0].year.is_none());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() {
    if let Err(error) = gateway::run().await {
        eprintln!("MeshGraph gateway failed: {error}");
        std::process::exit(1);
    }
}
