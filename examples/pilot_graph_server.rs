//! Ephemeral Oxigraph service for local integration tests when Docker is unavailable.
//! Data is kept in memory and discarded on exit. Production/local persistence uses Compose.
#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use axum::{extract::State, http::StatusCode, routing::post, Router};
    use oxigraph::{
        io::{RdfFormat, RdfParser},
        model::NamedNode,
        sparql::results::QueryResultsFormat,
        store::Store,
    };
    use std::fs::File;

    let store = Store::new()?;
    for (file, graph) in [
        ("data/citations.ttl", "citations"),
        ("data/jpl-history/history.ttl", "jpl-history"),
    ] {
        store.load_from_reader(
            RdfParser::from_format(RdfFormat::Turtle).with_default_graph(NamedNode::new(format!(
                "http://example.org/meshgraph/graphs/{graph}"
            ))?),
            File::open(file)?,
        )?;
    }
    async fn query(
        State(store): State<Store>,
        body: String,
    ) -> Result<([(&'static str, &'static str); 1], Vec<u8>), (StatusCode, String)> {
        let results = store
            .query(body.as_str())
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        let bytes = results
            .write(Vec::new(), QueryResultsFormat::Json)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        Ok(([("content-type", "application/sparql-results+json")], bytes))
    }
    let app = Router::new().route("/query", post(query)).with_state(store);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:7878").await?;
    println!("Ephemeral pilot Oxigraph test service at http://127.0.0.1:7878/query");
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
