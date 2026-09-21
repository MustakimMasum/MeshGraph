import { writeFile } from "node:fs/promises";

const OPENALEX_WORKS_URL = "https://api.openalex.org/works";
const SEED_DOI = "10.18653/v1/N19-1423";
const TARGET_NODE_COUNT = 50;
const OUTPUT_PATH = new URL("../data/citations.ttl", import.meta.url);
const SELECT = [
  "id",
  "doi",
  "title",
  "publication_year",
  "type",
  "cited_by_count",
  "primary_topic",
  "abstract_inverted_index",
  "referenced_works",
  "open_access",
  "best_oa_location",
].join(",");

async function fetchJson(url) {
  const requestUrl = new URL(url);
  const email = process.env.OPENALEX_EMAIL?.trim();
  if (email) requestUrl.searchParams.set("mailto", email);

  const response = await fetch(requestUrl, {
    headers: { "User-Agent": "MeshGraph seed generator/1.0" },
  });
  if (!response.ok) {
    throw new Error(`OpenAlex returned ${response.status} for ${requestUrl}`);
  }
  return response.json();
}

async function fetchSeed() {
  const url = new URL(`${OPENALEX_WORKS_URL}/doi:${SEED_DOI}`);
  url.searchParams.set("select", SELECT);
  return fetchJson(url);
}

async function fetchWorks(ids) {
  const works = [];
  for (let offset = 0; offset < ids.length; offset += 25) {
    const batch = ids.slice(offset, offset + 25).map(compactId);
    const url = new URL(OPENALEX_WORKS_URL);
    url.searchParams.set("filter", `openalex_id:${batch.join("|")}`);
    url.searchParams.set("per-page", "100");
    url.searchParams.set("select", SELECT);
    const response = await fetchJson(url);
    works.push(...response.results);
  }
  return works;
}

async function fetchCitingWorks(seedId) {
  const url = new URL(OPENALEX_WORKS_URL);
  url.searchParams.set("filter", `cites:${compactId(seedId)}`);
  url.searchParams.set("sort", "cited_by_count:desc");
  url.searchParams.set("per-page", "50");
  url.searchParams.set("select", SELECT);
  const response = await fetchJson(url);
  return response.results;
}

function compactId(iri) {
  return iri.slice(iri.lastIndexOf("/") + 1);
}

function reconstructAbstract(index) {
  if (!index) return null;
  const words = [];
  for (const [word, positions] of Object.entries(index)) {
    for (const position of positions) words[position] = word;
  }
  return words.join(" ");
}

function turtleString(value) {
  return `"${String(value)
    .replaceAll("\\", "\\\\")
    .replaceAll('"', '\\"')
    .replaceAll("\r", "\\r")
    .replaceAll("\n", "\\n")}"`;
}

function preferredUrl(work) {
  return (
    work.best_oa_location?.pdf_url ??
    work.open_access?.oa_url ??
    work.best_oa_location?.landing_page_url ??
    null
  );
}

function internalDegree(work, referenceIds, citedBySelected) {
  const outgoing = work.referenced_works.filter((id) => referenceIds.has(id)).length;
  return outgoing + (citedBySelected.get(work.id) ?? 0);
}

function selectNeighbors(works) {
  const referenceIds = new Set(works.map((work) => work.id));
  const citedBySelected = new Map();
  for (const work of works) {
    for (const target of work.referenced_works) {
      if (referenceIds.has(target)) {
        citedBySelected.set(target, (citedBySelected.get(target) ?? 0) + 1);
      }
    }
  }

  return works
    .toSorted((left, right) => {
      const degreeDifference =
        internalDegree(right, referenceIds, citedBySelected) -
        internalDegree(left, referenceIds, citedBySelected);
      return (
        degreeDifference ||
        (right.cited_by_count ?? 0) - (left.cited_by_count ?? 0) ||
        left.id.localeCompare(right.id)
      );
    })
    .slice(0, TARGET_NODE_COUNT - 1);
}

function renderWork(work, selectedIds, isSeed) {
  const predicates = [
    "a schema:ScholarlyArticle",
    `schema:name ${turtleString(work.title ?? "Untitled paper")}`,
  ];
  if (work.publication_year) {
    predicates.push(`dcterms:created "${work.publication_year}"^^xsd:gYear`);
  }
  if (work.type) predicates.push(`schema:genre ${turtleString(work.type)}`);
  if (work.primary_topic?.display_name) {
    predicates.push(`schema:about ${turtleString(work.primary_topic.display_name)}`);
  }
  const abstract = reconstructAbstract(work.abstract_inverted_index);
  if (abstract) predicates.push(`schema:abstract ${turtleString(abstract)}`);
  if (work.doi) predicates.push(`schema:identifier ${turtleString(work.doi)}`);
  const url = preferredUrl(work);
  if (url) predicates.push(`schema:url <${url}>`);
  predicates.push(`ex:citationCount "${work.cited_by_count ?? 0}"^^xsd:integer`);

  const citations = work.referenced_works.filter((id) => selectedIds.has(id));
  if (citations.length) {
    citations.sort();
    predicates.push(`cito:cites ${citations.map((id) => `<${id}>`).join(", ")}`);
  }
  if (isSeed) predicates.push("ex:isSeed true");

  return `<${work.id}>\n    ${predicates.join(" ;\n    ")} .`;
}

const seed = await fetchSeed();
const references = await fetchWorks(seed.referenced_works);
const citingWorks = await fetchCitingWorks(seed.id);
const neighborsById = new Map(
  [...references, ...citingWorks]
    .filter((work) => work.id !== seed.id)
    .map((work) => [work.id, work]),
);
const selectedNeighbors = selectNeighbors([...neighborsById.values()]);
if (selectedNeighbors.length !== TARGET_NODE_COUNT - 1) {
  throw new Error(
    `Expected ${TARGET_NODE_COUNT - 1} neighbors, received ${selectedNeighbors.length}`,
  );
}

const works = [seed, ...selectedNeighbors];
const selectedIds = new Set(works.map((work) => work.id));
const edgeCount = works.reduce(
  (count, work) =>
    count + work.referenced_works.filter((id) => selectedIds.has(id)).length,
  0,
);
const header = `# Generated from OpenAlex by scripts/generate-citation-seed.mjs.\n# Seed: BERT: Pre-training of Deep Bidirectional Transformers for Language Understanding\n# ${works.length} papers and ${edgeCount} citation relationships.\n\n@prefix schema: <https://schema.org/> .\n@prefix cito: <http://purl.org/spar/cito/> .\n@prefix dcterms: <http://purl.org/dc/terms/> .\n@prefix ex: <http://example.org/meshgraph/> .\n@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n\n`;
const turtle = `${header}${works
  .map((work, index) => renderWork(work, selectedIds, index === 0))
  .join("\n\n")}\n`;

await writeFile(OUTPUT_PATH, turtle, "utf8");
console.log(`Wrote ${works.length} papers and ${edgeCount} links to data/citations.ttl`);
