#!/bin/sh
set -eu

database_url="${OXIGRAPH_URL:-http://oxigraph_db:7878}"
force_seed="${FORCE_SEED:-false}"
case "$force_seed" in
    true|false) ;;
    *) echo 'FORCE_SEED must be true or false' >&2; exit 1 ;;
esac

# Bound startup waiting and HTTP calls so a failed service fails the seed job.
attempt=0
until curl --fail --silent --show-error --max-time 5 \
    --get --data-urlencode 'query=ASK {}' "$database_url/query" >/dev/null; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 30 ]; then
        echo 'Oxigraph did not become ready' >&2
        exit 1
    fi
    sleep 2
done

seed_graph() {
    collection="$1"
    file="$2"
    graph="http://example.org/meshgraph/graphs/$collection"
    result=$(curl --fail --silent --show-error --max-time 15 \
        --header 'Accept: application/sparql-results+json' \
        --get --data-urlencode "query=ASK { GRAPH <$graph> { ?s ?p ?o } }" \
        "$database_url/query" | tr -d '[:space:]')
    case "$result" in
        *'"boolean":true'*)
            if [ "$force_seed" = false ]; then
                echo "Keeping existing $collection graph"
                return
            fi ;;
        *'"boolean":false'*) ;;
        *) echo "Invalid Oxigraph response for $collection: $result" >&2; exit 1 ;;
    esac
    echo "Loading $collection from $file"
    curl --fail --silent --show-error --max-time 120 --request PUT \
        --header 'Content-Type: text/turtle' --data-binary "@$file" \
        --url-query "graph=$graph" "$database_url/store"
}

seed_graph citations /seed/citations.ttl
seed_graph jpl-history /seed/jpl-history/history.ttl
