#!/bin/sh
# Run from the repository root: sudo sh deploy/backup.sh COMPOSE_FILE OUTPUT.tar.gz
set -eu
umask 077

if [ "$#" -ne 2 ]; then
    echo 'Usage: sh deploy/backup.sh docker-compose.cloud.yml /absolute/path/graph.tar.gz' >&2
    exit 1
fi
compose_file="$1"
output="$2"
case "$compose_file" in
    docker-compose.cloud.yml|docker-compose.vps.yml) ;;
    *) echo 'Choose docker-compose.cloud.yml or docker-compose.vps.yml' >&2; exit 1 ;;
esac
case "$output" in
    /*.tar.gz) ;;
    *) echo 'Output must be an absolute path ending in .tar.gz' >&2; exit 1 ;;
esac
compose() {
    docker compose --env-file .env.cloud -f "$compose_file" "$@"
}
compose config --quiet
database_container=$(compose ps --all --quiet oxigraph_db)
if [ -z "$database_container" ]; then
    echo 'Database container does not exist; deploy the stack first' >&2
    exit 1
fi
# Use the persistent volume identity, so different checkouts and Compose files
# targeting the same database share the same lock. Hold it through the EXIT trap.
data_volume=$(docker inspect --format '{{range .Mounts}}{{if eq .Destination "/data"}}{{.Name}}{{end}}{{end}}' "$database_container")
case "$data_volume" in
    ''|*[!a-zA-Z0-9_.-]*) echo 'Could not identify the named database volume' >&2; exit 1 ;;
esac
if ! command -v flock >/dev/null 2>&1; then
    echo 'Install util-linux (flock) before running backups' >&2
    exit 1
fi
lock_dir="${MESHGRAPH_BACKUP_LOCK_DIR:-/var/lock/meshgraph}"
mkdir -p "$lock_dir"
exec 9>"$lock_dir/$data_volume.lock"
if ! flock -n 9; then
    echo "A backup of $data_volume is already running; this run was skipped" >&2
    exit 75
fi
if [ -e "$output" ] || [ -e "$output.partial" ]; then
    echo 'Output or partial output already exists; choose a new filename' >&2
    exit 1
fi

for service in oxigraph_db axum_gateway; do
    container=$(compose ps --quiet "$service")
    if [ -z "$container" ] || [ "$(docker inspect --format '{{.State.Running}}' "$container")" != true ]; then
        echo "Service $service must be running before taking this backup" >&2
        exit 1
    fi
done

# Download the helper before downtime begins.
docker pull alpine:3 >/dev/null
restart_services() {
    compose start oxigraph_db axum_gateway
}
trap restart_services EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
compose stop axum_gateway oxigraph_db
docker run --rm --network none --volumes-from "$database_container:ro" \
    alpine:3 tar -C /data -czf - . > "$output.partial"
gzip -t "$output.partial"
mv "$output.partial" "$output"
echo "Database backup saved to $output"
