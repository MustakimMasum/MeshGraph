use std::collections::{HashMap, HashSet, VecDeque};

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::history_model::{HistoryDataset, HistoryLink};

const QUERY: &str = "SELECT ?payload WHERE { GRAPH <http://example.org/meshgraph/graphs/jpl-history> { <http://example.org/meshgraph/graphs/jpl-history> <http://example.org/meshgraph/history/payload> ?payload } } LIMIT 1";
type Failure = (StatusCode, Json<Value>);
type ResultJson = Result<Json<Value>, Failure>;

#[derive(Clone)]
struct HistoryState {
    client: reqwest::Client,
    query_url: String,
}

pub fn router(query_url: String, client: reqwest::Client) -> Router {
    Router::new()
        .route("/api/v1/history/summary", get(summary))
        .route("/api/v1/history/graph", get(graph))
        .route("/api/v1/history/neighborhood", get(neighborhood))
        .route("/api/v1/history/nodes/:id", get(node))
        .route("/api/v1/history/links/:id", get(link))
        .route("/api/v1/history/search", get(search))
        .route("/api/v1/history/path", get(path))
        .route("/api/v1/history/attachments/:id", get(attachment))
        .route("/api/v1/history/quality", get(quality))
        .with_state(HistoryState { client, query_url })
}

fn failure(status: StatusCode, message: impl Into<String>) -> Failure {
    (status, Json(json!({"error": message.into()})))
}

async fn load(state: &HistoryState) -> Result<HistoryDataset, Failure> {
    let response = state
        .client
        .post(&state.query_url)
        .header("content-type", "application/sparql-query")
        .header("accept", "application/sparql-results+json")
        .body(QUERY)
        .send()
        .await
        .map_err(|e| failure(StatusCode::BAD_GATEWAY, e.to_string()))?
        .error_for_status()
        .map_err(|e| failure(StatusCode::BAD_GATEWAY, e.to_string()))?;
    let result: Value = response
        .json()
        .await
        .map_err(|e| failure(StatusCode::BAD_GATEWAY, e.to_string()))?;
    let payload = result.pointer("/results/bindings/0/payload/value").and_then(Value::as_str)
        .ok_or_else(|| failure(StatusCode::SERVICE_UNAVAILABLE, "JPL History has not been imported. Run the documented history import and seed steps."))?;
    serde_json::from_str(payload).map_err(|e| {
        failure(
            StatusCode::BAD_GATEWAY,
            format!("Invalid history dataset: {e}"),
        )
    })
}

async fn summary(State(state): State<HistoryState>) -> ResultJson {
    let data = load(&state).await?;
    Ok(Json(
        json!({"rootId": data.root_id, "version": data.version, "counts": data.quality["counts"], "title": "JPL History"}),
    ))
}

async fn graph(State(state): State<HistoryState>) -> ResultJson {
    Ok(Json(json!(load(&state).await?)))
}

async fn quality(State(state): State<HistoryState>) -> ResultJson {
    Ok(Json(load(&state).await?.quality))
}

fn validate_id(id: &str) -> Result<(), Failure> {
    if id.len() != 36
        || !id.bytes().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
    {
        return Err(failure(StatusCode::BAD_REQUEST, "Expected a source UUID"));
    }
    Ok(())
}

async fn node(State(state): State<HistoryState>, Path(id): Path<String>) -> ResultJson {
    validate_id(&id)?;
    let data = load(&state).await?;
    let node = data
        .nodes
        .iter()
        .find(|node| node.id == id)
        .ok_or_else(|| failure(StatusCode::NOT_FOUND, "Unknown history node"))?;
    Ok(Json(
        json!({"node": node, "attachments": data.attachments.iter().filter(|a| a.owner == id).collect::<Vec<_>>(), "links": data.links.iter().filter(|l| l.source == id || l.target == id).collect::<Vec<_>>()}),
    ))
}

async fn link(State(state): State<HistoryState>, Path(id): Path<String>) -> ResultJson {
    validate_id(&id)?;
    let data = load(&state).await?;
    let edge = data
        .links
        .iter()
        .find(|edge| edge.id == id)
        .ok_or_else(|| failure(StatusCode::NOT_FOUND, "Unknown history relationship"))?;
    Ok(Json(json!(edge)))
}

#[derive(Default, Deserialize)]
struct Options {
    id: Option<String>,
    q: Option<String>,
    from: Option<String>,
    to: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    organizational: Option<bool>,
}

fn allowed(link: &HistoryLink, organizational: bool) -> bool {
    organizational || (link.meaning == 1 && matches!(link.relation, 1 | 3))
}

async fn neighborhood(
    State(state): State<HistoryState>,
    Query(options): Query<Options>,
) -> ResultJson {
    let data = load(&state).await?;
    let id = options.id.unwrap_or_else(|| data.root_id.clone());
    validate_id(&id)?;
    if !data.nodes.iter().any(|n| n.id == id) {
        return Err(failure(StatusCode::NOT_FOUND, "Unknown history node"));
    }
    let limit = options.limit.unwrap_or(60).clamp(1, 300);
    let mut ids = HashSet::from([id.clone()]);
    for edge in data
        .links
        .iter()
        .filter(|e| allowed(e, options.organizational.unwrap_or(false)))
    {
        if edge.source == id {
            ids.insert(edge.target.clone());
        }
        if edge.target == id {
            ids.insert(edge.source.clone());
        }
    }
    let total = ids.len();
    let mut ordered: Vec<_> = ids.into_iter().filter(|other| other != &id).collect();
    ordered.sort();
    ordered.truncate(limit.saturating_sub(1));
    ordered.push(id);
    let ids: HashSet<_> = ordered.into_iter().collect();
    Ok(Json(
        json!({"nodes": data.nodes.iter().filter(|n| ids.contains(&n.id)).collect::<Vec<_>>(), "links": data.links.iter().filter(|e| ids.contains(&e.source) && ids.contains(&e.target) && allowed(e, options.organizational.unwrap_or(false))).collect::<Vec<_>>(), "total": total, "truncated": total > limit}),
    ))
}

async fn search(State(state): State<HistoryState>, Query(options): Query<Options>) -> ResultJson {
    let query = options.q.unwrap_or_default().trim().to_lowercase();
    if query.is_empty() || query.len() > 512 {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            "Search requires 1–512 bytes of text",
        ));
    }
    let data = load(&state).await?;
    let matches: Vec<_> = data
        .nodes
        .iter()
        .filter(|n| {
            n.title.to_lowercase().contains(&query) || n.notes.to_lowercase().contains(&query)
        })
        .collect();
    let total = matches.len();
    let results: Vec<_> = matches.into_iter().skip(options.offset.unwrap_or(0)).take(options.limit.unwrap_or(30).clamp(1, 100)).map(|n| {
        let lower = n.notes.to_lowercase();
        let start = lower.find(&query).map(|offset| lower[..offset].chars().count().saturating_sub(60)).unwrap_or(0);
        json!({"id": n.id, "title": n.title, "types": n.types, "excerpt": n.notes.chars().skip(start).take(280).collect::<String>()})
    }).collect();
    Ok(Json(json!({"results": results, "total": total})))
}

fn connection_path(data: &HistoryDataset, from: &str, to: &str) -> Value {
    let mut queue = VecDeque::from([from.to_owned()]);
    let mut seen = HashSet::from([from.to_owned()]);
    let mut parents: HashMap<String, (String, String, bool)> = HashMap::new();
    while let Some(id) = queue.pop_front() {
        if id == to {
            break;
        }
        for edge in data.links.iter().filter(|edge| allowed(edge, false)) {
            let (next, reverse) = if edge.source == id {
                (&edge.target, false)
            } else if edge.target == id {
                (&edge.source, true)
            } else {
                continue;
            };
            if seen.insert(next.clone()) {
                parents.insert(next.clone(), (id.clone(), edge.id.clone(), reverse));
                queue.push_back(next.clone());
            }
        }
    }
    let mut nodes = vec![];
    let mut steps = vec![];
    if seen.contains(to) {
        let mut cursor = to.to_owned();
        nodes.push(cursor.clone());
        while let Some((parent, link, reverse)) = parents.get(&cursor) {
            steps.push(json!({"linkId": link, "from": parent, "to": cursor, "reverse": reverse}));
            cursor = parent.clone();
            nodes.push(cursor.clone());
        }
        nodes.reverse();
        steps.reverse();
    }
    json!({"found": seen.contains(to), "nodes": nodes, "steps": steps, "policy": "Ordinary hierarchy and cross-links, traversed in either direction. System, tag, and type links excluded. Connectivity does not establish causation.", "limited": false})
}

async fn path(State(state): State<HistoryState>, Query(options): Query<Options>) -> ResultJson {
    let from = options.from.unwrap_or_default();
    let to = options.to.unwrap_or_default();
    validate_id(&from)?;
    validate_id(&to)?;
    let data = load(&state).await?;
    if ![&from, &to]
        .iter()
        .all(|id| data.nodes.iter().any(|n| &n.id == *id))
    {
        return Err(failure(StatusCode::NOT_FOUND, "Unknown path endpoint"));
    }
    Ok(Json(connection_path(&data, &from, &to)))
}

async fn attachment(
    State(state): State<HistoryState>,
    Path(id): Path<String>,
) -> Result<Response, Failure> {
    validate_id(&id)?;
    let data = load(&state).await?;
    let item = data
        .attachments
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| failure(StatusCode::NOT_FOUND, "Unknown attachment"))?;
    if item.status != "available" {
        return Err(failure(
            StatusCode::NOT_FOUND,
            "Attachment content is not included in this export",
        ));
    }
    if let Some(asset) = &item.asset {
        let suffix = asset
            .strip_prefix(&id)
            .ok_or_else(|| failure(StatusCode::BAD_GATEWAY, "Invalid attachment manifest"))?;
        let mime = match suffix {
            ".png" => "image/png",
            ".jpg" | ".jpeg" => "image/jpeg",
            ".webp" => "image/webp",
            _ => {
                return Err(failure(
                    StatusCode::BAD_GATEWAY,
                    "Unsupported attachment asset",
                ))
            }
        };
        let bytes = tokio::fs::read(format!("dist/public/history-assets/{asset}"))
            .await
            .map_err(|_| {
                failure(
                    StatusCode::NOT_FOUND,
                    "Attachment asset is not installed; rebuild the frontend after import",
                )
            })?;
        return Ok((
            [
                (header::CONTENT_TYPE, mime),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            bytes,
        )
            .into_response());
    }
    Ok((
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        item.text.clone(),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_rdf_roundtrips_and_repeated_load_does_not_duplicate() {
        use oxigraph::{
            io::{RdfFormat, RdfParser},
            model::NamedNode,
            sparql::QueryResults,
            store::Store,
        };
        let store = Store::new().expect("memory store");
        for _ in 0..2 {
            let parser = RdfParser::from_format(RdfFormat::Turtle).with_default_graph(
                NamedNode::new("http://example.org/meshgraph/graphs/jpl-history")
                    .expect("graph IRI"),
            );
            store
                .load_from_reader(
                    parser,
                    include_bytes!("../data/jpl-history/history.ttl").as_slice(),
                )
                .expect("valid generated RDF");
        }
        let QueryResults::Solutions(mut solutions) = store.query(QUERY).expect("query parses")
        else {
            panic!("expected solutions")
        };
        let row = solutions.next().expect("snapshot").expect("valid row");
        let Some(oxigraph::model::Term::Literal(value)) = row.get("payload") else {
            panic!("expected literal")
        };
        let dataset: HistoryDataset =
            serde_json::from_str(value.value()).expect("snapshot roundtrip");
        assert_eq!(
            (
                dataset.nodes.len(),
                dataset.links.len(),
                dataset.attachments.len()
            ),
            (262, 418, 58)
        );
        assert!(solutions.next().is_none());
        assert_eq!(
            store.len().expect("quad count"),
            1 + 262 * 3 + 418 * 6 + 58 * 3
        );
    }

    #[tokio::test]
    async fn missing_snapshot_and_invalid_payload_map_to_structured_errors() {
        for (body, expected) in [
            (
                json!({"results":{"bindings":[]}}),
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                json!({"results":{"bindings":[{"payload":{"value":"invalid"}}]}}),
                StatusCode::BAD_GATEWAY,
            ),
        ] {
            let app = Router::new().route(
                "/query",
                axum::routing::post(move || {
                    let body = body.clone();
                    async move { Json(body) }
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("test port");
            let address = listener.local_addr().expect("address");
            let task = tokio::spawn(async move {
                axum::serve(listener, app).await.expect("test service");
            });
            let error = load(&HistoryState {
                client: reqwest::Client::new(),
                query_url: format!("http://{address}/query"),
            })
            .await
            .expect_err("structured failure");
            task.abort();
            assert_eq!(error.0, expected);
            assert!(error.1 .0["error"].is_string());
        }
    }

    #[test]
    fn rejects_paths_and_malformed_identifiers() {
        assert!(validate_id("../../secret").is_err());
        assert!(validate_id("6b8f6622-c037-54f0-9c09-b98a6c2428f0").is_ok());
    }

    #[test]
    fn path_excludes_type_shortcuts_and_records_reverse_traversal() {
        let edge = |id: &str, a: &str, b: &str, meaning| HistoryLink {
            id: id.into(),
            source: a.into(),
            target: b.into(),
            relation: 1,
            meaning,
            direction: -1,
            name: String::new(),
            kind: String::new(),
            meaning_label: String::new(),
            direction_label: String::new(),
            raw: Value::Null,
        };
        let data = HistoryDataset {
            schema_version: 1,
            version: String::new(),
            brain_id: String::new(),
            root_id: "a".into(),
            nodes: vec![],
            links: vec![
                edge("type", "a", "c", 2),
                edge("one", "b", "a", 1),
                edge("two", "b", "c", 1),
            ],
            attachments: vec![],
            quality: Value::Null,
            meta: Value::Null,
        };
        let path = connection_path(&data, "a", "c");
        assert_eq!(path["nodes"], json!(["a", "b", "c"]));
        assert_eq!(path["steps"][0]["reverse"], true);
        assert_eq!(connection_path(&data, "a", "z")["found"], false);
    }
}
