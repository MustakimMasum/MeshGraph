use std::collections::{HashMap, HashSet, VecDeque};

use oxigraph::model::{Literal, NamedNode};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const OPENALEX_WORKS_URL: &str = "https://api.openalex.org/works";
const OPENALEX_SELECT: &str = "id,doi,title,publication_year,type,cited_by_count,primary_topic,abstract_inverted_index,referenced_works,open_access,best_oa_location";
const CITATION_GRAPH: &str = "http://example.org/meshgraph/graphs/citations";
const MAX_DEPTH: u8 = 3;
const MAX_WORKS: usize = 250;

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const SCHEMA_ARTICLE: &str = "https://schema.org/ScholarlyArticle";
const SCHEMA_NAME: &str = "https://schema.org/name";
const SCHEMA_GENRE: &str = "https://schema.org/genre";
const SCHEMA_ABOUT: &str = "https://schema.org/about";
const SCHEMA_ABSTRACT: &str = "https://schema.org/abstract";
const SCHEMA_IDENTIFIER: &str = "https://schema.org/identifier";
const SCHEMA_URL: &str = "https://schema.org/url";
const DCTERMS_CREATED: &str = "http://purl.org/dc/terms/created";
const CITO_CITES: &str = "http://purl.org/spar/cito/cites";
const EX_CITATION_COUNT: &str = "http://example.org/meshgraph/citationCount";
const XSD_GYEAR: &str = "http://www.w3.org/2001/XMLSchema#gYear";
const XSD_INTEGER: &str = "http://www.w3.org/2001/XMLSchema#integer";

#[derive(Debug, Deserialize)]
struct OpenAlexWork {
    id: String,
    doi: Option<String>,
    title: Option<String>,
    publication_year: Option<i32>,
    #[serde(rename = "type")]
    work_type: Option<String>,
    cited_by_count: Option<u64>,
    primary_topic: Option<OpenAlexTopic>,
    abstract_inverted_index: Option<HashMap<String, Vec<usize>>>,
    #[serde(default)]
    referenced_works: Vec<String>,
    open_access: Option<OpenAccess>,
    best_oa_location: Option<OpenAlexLocation>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexTopic {
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct OpenAccess {
    oa_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexLocation {
    pdf_url: Option<String>,
    landing_page_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexWorkList {
    results: Vec<OpenAlexWork>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestionSummary {
    pub seed_id: String,
    pub requested_depth: u8,
    pub works_ingested: usize,
    pub citation_links: usize,
    pub truncated: bool,
}

#[derive(Debug)]
pub enum IngestionError {
    InvalidIdentifier(String),
    OpenAlex(String),
    Rdf(String),
}

impl std::fmt::Display for IngestionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIdentifier(message) | Self::OpenAlex(message) | Self::Rdf(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl std::error::Error for IngestionError {}

pub async fn ingest_openalex_network(
    client: &Client,
    update_url: &str,
    identifier: &str,
    requested_depth: u8,
    contact_email: Option<&str>,
) -> Result<IngestionSummary, IngestionError> {
    let seed = normalize_identifier(identifier)?;
    let depth = requested_depth.min(MAX_DEPTH);
    let mut queue = VecDeque::from([(seed, 0_u8)]);
    let mut queued = HashSet::new();
    let mut visited = HashSet::new();
    let mut statements = HashSet::new();
    let mut seed_id = String::new();
    let mut citation_links = 0;

    while let Some((work_identifier, current_depth)) = queue.pop_front() {
        if visited.len() >= MAX_WORKS {
            break;
        }
        if !visited.insert(work_identifier.clone()) {
            continue;
        }

        let work = fetch_work(client, &work_identifier, contact_email).await?;
        let subject = named_node(&work.id)?;
        if seed_id.is_empty() {
            seed_id = compact_openalex_id(&work.id);
        }
        append_work_statements(&mut statements, &subject, &work)?;

        for reference in &work.referenced_works {
            let object = named_node(reference)?;
            statements.insert(statement_named(&subject, CITO_CITES, &object)?);
            statements.insert(statement_named_value(reference, RDF_TYPE, SCHEMA_ARTICLE)?);
            citation_links += 1;

            if current_depth < depth && queued.insert(reference.clone()) {
                queue.push_back((reference.clone(), current_depth + 1));
            }
        }
    }

    let mut statements: Vec<String> = statements.into_iter().collect();
    statements.sort_unstable();
    persist_statements(client, update_url, &statements).await?;

    Ok(IngestionSummary {
        seed_id,
        requested_depth: depth,
        works_ingested: visited.len(),
        citation_links,
        truncated: !queue.is_empty(),
    })
}

fn normalize_identifier(identifier: &str) -> Result<String, IngestionError> {
    let value = identifier.trim();
    if value.is_empty() || value.len() > 300 || value.chars().any(char::is_control) {
        return Err(IngestionError::InvalidIdentifier(
            "A DOI or arXiv identifier is required".to_owned(),
        ));
    }

    let lower = value.to_ascii_lowercase();
    let arxiv_doi_id = lower
        .find("arxiv.")
        .and_then(|index| value.get(index + "arxiv.".len()..));
    let normalized = if lower.starts_with("https://openalex.org/w") {
        value.to_owned()
    } else if lower.starts_with("w") && value[1..].chars().all(|value| value.is_ascii_digit()) {
        format!("https://openalex.org/{value}")
    } else if lower.starts_with("https://doi.org/10.48550/arxiv.")
        || lower.starts_with("http://doi.org/10.48550/arxiv.")
    {
        format!("arxiv:{}", arxiv_doi_id.unwrap_or_default())
    } else if lower.starts_with("https://doi.org/") || lower.starts_with("http://doi.org/") {
        format!("doi:{}", value.splitn(4, '/').nth(3).unwrap_or_default())
    } else if lower.starts_with("doi:") {
        format!("doi:{}", value[4..].trim())
    } else if lower.starts_with("10.48550/arxiv.") {
        format!("arxiv:{}", arxiv_doi_id.unwrap_or_default())
    } else if value.starts_with("10.") && value.contains('/') {
        format!("doi:{value}")
    } else if lower.starts_with("https://arxiv.org/abs/") {
        format!("arxiv:{}", value.rsplit('/').next().unwrap_or_default())
    } else if lower.starts_with("arxiv:") {
        format!("arxiv:{}", value[6..].trim())
    } else {
        format!("arxiv:{value}")
    };

    Ok(normalized)
}

async fn fetch_work(
    client: &Client,
    identifier: &str,
    contact_email: Option<&str>,
) -> Result<OpenAlexWork, IngestionError> {
    if let Some(arxiv_id) = identifier.strip_prefix("arxiv:") {
        return fetch_arxiv_work(client, arxiv_id, contact_email).await;
    }

    let lookup_id = identifier
        .strip_prefix("https://openalex.org/")
        .unwrap_or(identifier);
    let mut request = client
        .get(format!("{OPENALEX_WORKS_URL}/{lookup_id}"))
        .query(&[("select", OPENALEX_SELECT)]);
    if let Some(email) = contact_email.filter(|email| !email.trim().is_empty()) {
        request = request.query(&[("mailto", email)]);
    }

    let response = request.send().await.map_err(|error| {
        IngestionError::OpenAlex(format!("OpenAlex request failed for {identifier}: {error}"))
    })?;
    let status = response.status();
    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        return Err(IngestionError::OpenAlex(format!(
            "OpenAlex returned {status} for {identifier}: {}",
            detail.chars().take(240).collect::<String>()
        )));
    }

    response.json::<OpenAlexWork>().await.map_err(|error| {
        IngestionError::OpenAlex(format!("OpenAlex returned invalid work data: {error}"))
    })
}

async fn fetch_arxiv_work(
    client: &Client,
    arxiv_id: &str,
    contact_email: Option<&str>,
) -> Result<OpenAlexWork, IngestionError> {
    let arxiv_id = strip_arxiv_version(arxiv_id);
    let filter = format!("locations.landing_page_url:http://arxiv.org/abs/{arxiv_id}");
    let mut request = client.get(OPENALEX_WORKS_URL).query(&[
        ("filter", filter.as_str()),
        ("per-page", "1"),
        ("select", OPENALEX_SELECT),
    ]);
    if let Some(email) = contact_email.filter(|email| !email.trim().is_empty()) {
        request = request.query(&[("mailto", email)]);
    }

    let response = request.send().await.map_err(|error| {
        IngestionError::OpenAlex(format!(
            "OpenAlex arXiv lookup failed for {arxiv_id}: {error}"
        ))
    })?;
    let status = response.status();
    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        return Err(IngestionError::OpenAlex(format!(
            "OpenAlex returned {status} for arXiv:{arxiv_id}: {}",
            detail.chars().take(240).collect::<String>()
        )));
    }
    response
        .json::<OpenAlexWorkList>()
        .await
        .map_err(|error| {
            IngestionError::OpenAlex(format!("OpenAlex returned invalid arXiv data: {error}"))
        })?
        .results
        .into_iter()
        .next()
        .ok_or_else(|| {
            IngestionError::OpenAlex(format!(
                "OpenAlex has no work with arXiv identifier {arxiv_id}"
            ))
        })
}

fn strip_arxiv_version(identifier: &str) -> &str {
    let Some(version_index) = identifier.rfind('v') else {
        return identifier;
    };
    if identifier[version_index + 1..]
        .chars()
        .all(|character| character.is_ascii_digit())
    {
        &identifier[..version_index]
    } else {
        identifier
    }
}

fn append_work_statements(
    statements: &mut HashSet<String>,
    subject: &NamedNode,
    work: &OpenAlexWork,
) -> Result<(), IngestionError> {
    statements.insert(statement_named_value(
        subject.as_str(),
        RDF_TYPE,
        SCHEMA_ARTICLE,
    )?);

    if let Some(title) = work.title.as_deref().filter(|value| !value.is_empty()) {
        statements.insert(statement_literal(
            subject,
            SCHEMA_NAME,
            Literal::new_simple_literal(title),
        )?);
    }
    if let Some(year) = work.publication_year {
        let datatype = named_node(XSD_GYEAR)?;
        statements.insert(statement_literal(
            subject,
            DCTERMS_CREATED,
            Literal::new_typed_literal(year.to_string(), datatype),
        )?);
    }
    if let Some(work_type) = work.work_type.as_deref() {
        statements.insert(statement_literal(
            subject,
            SCHEMA_GENRE,
            Literal::new_simple_literal(work_type),
        )?);
    }
    if let Some(topic) = work.primary_topic.as_ref() {
        statements.insert(statement_literal(
            subject,
            SCHEMA_ABOUT,
            Literal::new_simple_literal(&topic.display_name),
        )?);
    }
    if let Some(abstract_text) = reconstruct_abstract(work.abstract_inverted_index.as_ref()) {
        statements.insert(statement_literal(
            subject,
            SCHEMA_ABSTRACT,
            Literal::new_simple_literal(abstract_text),
        )?);
    }
    if let Some(doi) = work.doi.as_deref() {
        statements.insert(statement_literal(
            subject,
            SCHEMA_IDENTIFIER,
            Literal::new_simple_literal(doi),
        )?);
    }
    if let Some(url) = preferred_url(work) {
        statements.insert(statement_named_value(subject.as_str(), SCHEMA_URL, url)?);
    }
    if let Some(count) = work.cited_by_count {
        let datatype = named_node(XSD_INTEGER)?;
        statements.insert(statement_literal(
            subject,
            EX_CITATION_COUNT,
            Literal::new_typed_literal(count.to_string(), datatype),
        )?);
    }

    Ok(())
}

fn preferred_url(work: &OpenAlexWork) -> Option<&str> {
    work.best_oa_location
        .as_ref()
        .and_then(|location| location.pdf_url.as_deref())
        .or_else(|| {
            work.open_access
                .as_ref()
                .and_then(|access| access.oa_url.as_deref())
        })
        .or_else(|| {
            work.best_oa_location
                .as_ref()
                .and_then(|location| location.landing_page_url.as_deref())
        })
        .or(work.doi.as_deref())
}

fn reconstruct_abstract(index: Option<&HashMap<String, Vec<usize>>>) -> Option<String> {
    let index = index?;
    let mut words = Vec::new();
    for (word, positions) in index {
        for position in positions {
            words.push((*position, word.as_str()));
        }
    }
    words.sort_unstable_by_key(|(position, _)| *position);
    (!words.is_empty()).then(|| {
        words
            .into_iter()
            .map(|(_, word)| word)
            .collect::<Vec<_>>()
            .join(" ")
    })
}

async fn persist_statements(
    client: &Client,
    update_url: &str,
    statements: &[String],
) -> Result<(), IngestionError> {
    for chunk in statements.chunks(500) {
        let update = format!(
            "INSERT DATA {{ GRAPH <{CITATION_GRAPH}> {{\n{}\n}} }}",
            chunk.join("\n")
        );
        client
            .post(update_url)
            .header("content-type", "application/sparql-update")
            .body(update)
            .send()
            .await
            .map_err(|error| IngestionError::Rdf(format!("Oxigraph update failed: {error}")))?
            .error_for_status()
            .map_err(|error| IngestionError::Rdf(format!("Oxigraph rejected RDF data: {error}")))?;
    }
    Ok(())
}

fn compact_openalex_id(id: &str) -> String {
    id.rsplit('/').next().unwrap_or(id).to_owned()
}

fn named_node(value: &str) -> Result<NamedNode, IngestionError> {
    NamedNode::new(value)
        .map_err(|error| IngestionError::Rdf(format!("Invalid RDF IRI {value}: {error}")))
}

fn statement_named(
    subject: &NamedNode,
    predicate: &str,
    object: &NamedNode,
) -> Result<String, IngestionError> {
    let predicate = named_node(predicate)?;
    Ok(format!("{subject} {predicate} {object} ."))
}

fn statement_named_value(
    subject: &str,
    predicate: &str,
    object: &str,
) -> Result<String, IngestionError> {
    statement_named(&named_node(subject)?, predicate, &named_node(object)?)
}

fn statement_literal(
    subject: &NamedNode,
    predicate: &str,
    object: Literal,
) -> Result<String, IngestionError> {
    let predicate = named_node(predicate)?;
    Ok(format!("{subject} {predicate} {object} ."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_doi_and_arxiv_identifiers() {
        assert_eq!(
            normalize_identifier("10.1000/example").expect("valid DOI"),
            "doi:10.1000/example"
        );
        assert_eq!(
            normalize_identifier("arXiv:2401.12345").expect("valid arXiv ID"),
            "arxiv:2401.12345"
        );
        assert_eq!(
            normalize_identifier("10.48550/arXiv.1706.03762").expect("valid arXiv DOI"),
            "arxiv:1706.03762"
        );
    }

    #[test]
    fn reconstructs_openalex_abstract_in_position_order() {
        let index = HashMap::from([
            ("graph".to_owned(), vec![1]),
            ("Knowledge".to_owned(), vec![0]),
            ("works".to_owned(), vec![2]),
        ]);
        assert_eq!(
            reconstruct_abstract(Some(&index)).as_deref(),
            Some("Knowledge graph works")
        );
    }

    #[test]
    fn rdf_literals_are_escaped_by_oxigraph_types() {
        let subject = named_node("https://openalex.org/W1").expect("valid subject");
        let value = statement_literal(
            &subject,
            SCHEMA_NAME,
            Literal::new_simple_literal("A \"quoted\" title"),
        )
        .expect("valid statement");
        assert!(value.contains("\\\"quoted\\\""));
    }
}
