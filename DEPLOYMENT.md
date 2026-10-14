# MeshGraph VPS deployment handoff

This guide is for the professor's system administrator. It deploys the bundled
Citation Explorer and JPL History app on an existing Linux VPS. All server
commands below run in a Linux shell. No Rust, Node.js, or Trunk installation is
needed on the host: Docker builds the application.

## Independence from the developer's computer

The VPS hosts the frontend, gateway, persistent Oxigraph database, bundled
collections, history images, A-Frame, and webcam hand-tracking libraries/model.
The developer's computer can be powered off after the release is transferred.
No SSH tunnel, local gateway, local database, network file share, or connection
back to the developer's machine is configured or needed.

| Connection | Where it goes |
| --- | --- |
| Browser API and runtime assets | The deployed hostname, using same-origin paths |
| Gateway database queries and writes | `oxigraph_db:7878`, inside the VPS's Docker network |
| Gateway health check `localhost:3000` | Inside the gateway container |
| Existing proxy `127.0.0.1:3000` | The VPS host's own loopback interface |
| Optional Leap Motion `127.0.0.1:6437` | The visitor's own hardware bridge, only after choosing IR input |

Ordinary graph/history exploration and webcam model loading require no external
CDN calls. Optional **new citation imports** contact OpenAlex from the VPS, and
clicking a paper or archive web link opens the referenced external website.
Leap Motion is an optional hardware input; visitors can use mouse/keyboard,
webcam, or supported WebXR without running a local server.

Initial installation/builds still download Docker images and Rust dependencies.
Public HTTPS certificates need normal certificate-authority connectivity and
renewal. These external services are independent of the developer's machine.

The release includes `public/vendor/manifest.json` with checksums and source
URLs for bundled browser dependencies. The packaging command verifies those
checksums and rejects missing or changed assets.

## 1. Confirm the server and access requirements

- Recommended baseline: Ubuntu 24.04 LTS, **x86-64**, at least **2 vCPU, 4 GB RAM**,
  and **20 GB free disk** for images, compilation, and initial database storage.
  These are starting estimates for the current small dataset, not load-test results.
  More memory speeds builds; increase storage as the graph and image assets grow.
- SSH access and permission to run `sudo docker`. Docker Engine must support Linux
  containers and include the current Compose plugin (`docker compose`).
- A dedicated hostname, for example `meshgraph.university.edu`. Serve the app at
  the hostname root, not under `/meshgraph/`: frontend assets and API URLs use
  absolute paths.
- Outbound HTTPS access for image/build downloads and certificate renewal.
  Optional live citation ingestion also needs OpenAlex. Browser runtime libraries
  and webcam hand-tracking assets are included in the release and served by the VPS.
- Decide who may access the data. The app has no built-in login, and its citation
  ingestion API can write to the citation collection. For a private pilot, use
  the institution's access controls on the entire hostname through its proxy.

Choose **one** deployment mode before starting:

| VPS setup | Compose file | Public entry point |
| --- | --- | --- |
| Ports 80/443 are free; MeshGraph should manage HTTPS | `docker-compose.cloud.yml` | Included Caddy container |
| VPS already hosts websites with Nginx, Apache, Caddy, or a hosting panel | `docker-compose.vps.yml` | Existing host reverse proxy |

Both modes use the same app, seed job, and database volume. Do not launch both at
once. These commands use Compose project name `meshgraph`; keep it unchanged to
retain the same volume. If that name is already used, choose another fixed name
with `-p` consistently and adapt the backup procedure before deployment.

## 2. Check or install Docker

On a server where Docker is already installed:

```sh
sudo docker version
sudo docker compose version
df -h
free -h
sudo ss -ltnp
```

Confirm that the Docker **server** responds, there is adequate disk/RAM, and the
chosen ports are available. Avoid replacing an existing Docker installation on
a shared VPS without coordinating with its administrator.

For a new Ubuntu server without Docker, follow the
[official Docker Engine installation instructions](https://docs.docker.com/engine/install/ubuntu/).
For Ubuntu 24.04, the repository-based installation can be run as follows.
Use this only when Docker is absent and the administrator has checked for
conflicting container packages:

```sh
sudo apt-get update
sudo apt-get install -y curl ca-certificates unzip
sudo install -d -m 0755 /etc/apt/keyrings
sudo curl --fail --location https://download.docker.com/linux/ubuntu/gpg \
    --output /etc/apt/keyrings/docker.asc
sudo chmod 0644 /etc/apt/keyrings/docker.asc
meshgraph_arch=$(dpkg --print-architecture)
meshgraph_codename=$(. /etc/os-release; printf '%s' "$VERSION_CODENAME")
printf 'deb [arch=%s signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu %s stable\n' \
    "$meshgraph_arch" "$meshgraph_codename" | sudo tee /etc/apt/sources.list.d/docker.list >/dev/null
sudo apt-get update
sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin docker-buildx-plugin
```

Then verify:

```sh
sudo systemctl enable --now docker
sudo docker run --rm hello-world
sudo docker compose version
```

## 3. Transfer the release

The supplied `meshgraph-vps.zip` contains the application source, bundled RDF
data, frontend assets, deployment configuration, and this guide. It excludes the
developer's database, compiled assets, Git metadata, and private environment file.

From your own workstation, transfer it using your normal SSH credentials:

```sh
scp meshgraph-vps.zip meshgraph-vps.zip.sha256 ADMIN_USER@VPS_IP:/tmp/
```

On the VPS, use a new directory for the first deployment:

```sh
cd /tmp
sha256sum -c meshgraph-vps.zip.sha256
# Continue only when the checksum reports OK.
sudo mkdir -p /opt/meshgraph
sudo chown "$(id -un):$(id -gn)" /opt/meshgraph
cd /opt/meshgraph
unzip /tmp/meshgraph-vps.zip
test -f Cargo.lock
test -f data/citations.ttl
test -f data/jpl-history/history.ttl
test -f scripts/seed-graphs.sh
```

Install `unzip` through your OS package manager if needed. Alternatively, clone
the complete repository into `/opt/meshgraph` and ensure the prebuilt
`public/downloads/MeshGraphLeapSetup.exe` from the supplied release is present.
Building that optional Windows installer from source requires a Windows build
machine; the ZIP already contains it, so the VPS administrator needs no Windows
tools or connection to the developer's computer.
On an existing installation, back up first and follow the update section instead
of extracting over a live release.

## 4. Configure the hostname and mode

```sh
cd /opt/meshgraph
cp .env.cloud.example .env.cloud
chmod 600 .env.cloud
nano .env.cloud
```

Set `DOMAIN` to the real hostname, with no scheme, port, or path. The example
domain cannot obtain a certificate. `OPENALEX_EMAIL` is optional. Leave the other
defaults for the first deployment; `GATEWAY_PORT` only applies to existing-proxy mode.

Point the hostname's DNS **A record** to the VPS public IPv4 address. Add an AAAA
record only when public IPv6 is configured and reachable. For initial Caddy
certificate issuance, use DNS-only routing if an external CDN/proxy complicates
HTTP validation.

In this shell, choose the mode and define a command shorthand:

```sh
# Use this when MeshGraph manages HTTPS:
COMPOSE_FILE=docker-compose.cloud.yml
# OR use this when the existing host proxy manages HTTPS:
# COMPOSE_FILE=docker-compose.vps.yml

dc() {
    sudo docker compose --env-file .env.cloud -f "$COMPOSE_FILE" "$@"
}
dc config --quiet
```

Repeat `cd`, `COMPOSE_FILE=...`, and the `dc()` definition when opening a new SSH
shell. Use the selected file on every command; plain `docker compose up` starts
the development configuration instead.

## 5. Configure network access

### Mode A: included Caddy

Allow inbound TCP 80 and 443 for this VPS in the provider firewall and applicable
host firewall. UDP 443 is optional for HTTP/3. Retain SSH access according to the
institution's policy. Do not publish database port 7878 or gateway port 3000.

Caddy receives the hostname from `.env.cloud`, proxies to the gateway, redirects
HTTP to HTTPS, and stores certificate state in a persistent volume. DNS must
resolve to this server, and ports 80/443 must be reachable for automatic HTTPS.
See [Caddy's certificate prerequisites](https://caddyserver.com/docs/automatic-https).

### Mode B: existing reverse proxy

This mode starts no Caddy container. The gateway publishes only
`127.0.0.1:3000`; the database publishes no port. Change `GATEWAY_PORT` if another
application uses 3000 and use that same port in the proxy configuration.

Create a virtual host for the dedicated hostname through the VPS's normal
Nginx/Apache/Caddy or hosting-panel workflow. Obtain and renew its TLS certificate
using the existing certificate tooling, and redirect HTTP to HTTPS.

For **host-installed Nginx**, merge the location from
[`deploy/nginx.conf.example`](deploy/nginx.conf.example) into that hostname's
HTTPS server block. Preserve its existing certificate directives. The block
forwards all paths, including `/api/` and `.wasm`, to `http://127.0.0.1:3000`.
After the gateway is running, validate and reload:

```sh
sudo nginx -t
# Run only if validation passed:
sudo systemctl reload nginx
```

For **host-installed Caddy**, add this to its existing configuration, substituting
the hostname and configured gateway port:

```caddyfile
meshgraph.university.edu {
    reverse_proxy 127.0.0.1:3000
}
```

If the existing proxy runs **inside Docker**, its `127.0.0.1` refers to the proxy
container. Connect it to MeshGraph's `meshgraph_default` network and proxy to
`axum_gateway:3000` instead; make that network attachment persistent in the
proxy's Compose configuration. Keep port 7878 private. Do not stop the VPS's
other websites or add a second listener on their ports.

## 6. Build and start

```sh
dc up -d --build
dc ps -a
dc logs --tail=100 graph_seed axum_gateway
# Mode A only:
dc logs --tail=100 caddy
```

The initial build installs Trunk and compiles the frontend and gateway. An
uncached build can take tens of minutes on a small VPS; later builds reuse the
cached tooling layers. Build output goes to the SSH terminal. Normal application
logs use rotation to limit disk consumption.

Expected status:

- `oxigraph_db`: running.
- `graph_seed`: exited with code **0**, which is normal for this one-time job.
- `axum_gateway`: running and **healthy**.
- Mode A only, `caddy`: running after the gateway becomes healthy.

The gateway health check verifies that the HTTP server responds. The collection
checks below additionally verify database connectivity and seeded content.

If the compiler is killed or reports out-of-memory, set `CARGO_BUILD_JOBS=1` in
`.env.cloud` and rebuild. Add memory or administrator-managed swap if needed.
Do not run a second build concurrently. Rust is compiled in Docker; no host
`trunk build` is needed.

## 7. Verify the deployment

Set the actual hostname in the current shell:

```sh
APP_URL=https://meshgraph.university.edu
curl --fail --silent --show-error "$APP_URL/" -o /dev/null
curl --fail --silent --show-error "$APP_URL/api/v1/health" -o /dev/null
curl --fail --silent --show-error "$APP_URL/api/v1/citations/graph"
curl --fail --silent --show-error "$APP_URL/api/v1/history/summary"
```

All commands should succeed. Health returns **204** with an empty body. Citation
graph JSON should contain nonempty `nodes` and `links`. History summary should
contain the collection counts; the current bundled archive has **262 topics and
418 connections**.

In a desktop browser, check:

1. `https://YOUR_HOSTNAME/`: citation scene loads, nodes select, details display.
2. `https://YOUR_HOSTNAME/?collection=history`: history scene loads, search and
   topic notes work, and supplied images display.
3. Browser developer tools: no failed `.wasm`, `/public/`, or `/api/` requests.
4. HTTPS certificate is valid. Test webcam/VR only with permission and suitable
   hardware; rendering depends on the client browser/GPU. Leap Motion additionally
   needs the visitor's own Hyperion bridge described in the README; it does not
   connect to the developer's computer and is not needed to browse the app.

For Mode B, `curl --fail http://127.0.0.1:3000/api/v1/history/summary` on the VPS
helps separate gateway/database problems from proxy/TLS problems.

## 8. Confirm the pinned database image

### Optional Leap Motion setup for visitors

The bridge runs on the **Windows computer physically connected to the sensor**,
not on the VPS. It is not required for ordinary browsing or webcam controls.

1. Install Ultraleap Hyperion and connect the Leap Motion sensor on that computer.
2. In the deployed app, click **Leap Motion**. The app attempts to open the
   companion automatically and shows setup instructions when it cannot connect.
3. On first use, download **Windows companion** from the app and double-click
   `MeshGraphLeapSetup.exe`. This per-user GUI installer needs no command prompt.
4. Return to the app and click **Start Leap Motion** or its Leap Motion button.
   Allow the browser to open `MeshGraphLeap`, and approve this app's hostname in
   the companion's first-use dialog. The bridge starts in the background.
5. The app retries the connection and displays tracking status. Its button
   toggles hand input off when finished. The background bridge can remain running
   for the next session; closing it in Task Manager is optional.

The registered `meshgraph-leap://` handler accepts an application origin, not an
arbitrary command. The bridge binds only to loopback and permits approved web
origins. Hyperion must be installed locally; its driver cannot be embedded in a
browser. This one-time companion/driver installation and browser/Windows launch
prompts remain necessary. Institutions may need to approve the unsigned installer
and browser access to loopback. Only Windows x86-64 is currently supported.
The native bridge also uses the Microsoft Visual C++ x64 runtime; install the
institution's current Visual C++ Redistributable if Windows reports a missing
`VCRUNTIME140.dll`. The GUI launcher uses Windows' .NET Framework 4.x.

For a Hyperion DLL installed outside the standard path, the administrator can
set the user's `HYPERION_LEAPC_PATH` environment variable through Windows System
Properties. No daily command-line operation is needed.

### Record the deployed image

The supplied `.env.cloud.example` pins the exact official Oxigraph image used
in the release checks. Retain that value for the first deployment. After
validation, record the deployed image digest:

```sh
DB_CONTAINER=$(dc ps -q oxigraph_db)
DB_IMAGE_ID=$(sudo docker inspect --format '{{.Image}}' "$DB_CONTAINER")
sudo docker image inspect --format '{{index .RepoDigests 0}}' "$DB_IMAGE_ID"
```

Confirm that the `ghcr.io/oxigraph/oxigraph@sha256:...` value matches
`OXIGRAPH_IMAGE` in `.env.cloud`. If an administrator deliberately selected a
different image and validated it, record its digest there. The Compose fallback
when `OXIGRAPH_IMAGE` is omitted is `latest`; keep the explicit pin to avoid
unplanned database upgrades. Keep the deployed release and resolved config:

```sh
dc config > deployed-compose.yml
chmod 600 deployed-compose.yml
```

Keep database upgrades separate from app updates; take a backup and test the new
image against a restored copy before switching production.

## 9. Back up the database

The database is stored in Docker volume **`meshgraph_graph_data`**, not in the
application directory. The initial deployment starts with the bundled datasets;
it does not migrate the developer's `local_graph_store/`. A normal seed run keeps
existing collections, including live-ingested citations.

The included script makes a consistent filesystem backup by briefly stopping
the gateway and database. It restarts both on exit, including most failure cases.
Run it from `/opt/meshgraph` with the chosen Compose file:

```sh
sudo install -d -m 0700 /var/backups/meshgraph
BACKUP=/var/backups/meshgraph/graph-$(date -u +%Y%m%dT%H%M%SZ).tar.gz
sudo sh deploy/backup.sh "$COMPOSE_FILE" "$BACKUP"
dc ps -a
curl --fail --silent --show-error "$APP_URL/api/v1/history/summary"
```

Schedule it during a maintenance window. If power loss or `SIGKILL` interrupts
the backup, run `dc start oxigraph_db axum_gateway` and check the services.
Copy successful backups **off the VPS** using institutional backup storage.
Back up `.env.cloud`, the source release, and the proxy configuration separately.
Caddy certificate volumes survive normal container replacement; disk snapshots
can also preserve them. Saved viewpoints live in each user's browser and are
not part of the database backup.

### Restore drill on a separate server

Restore using the same release and pinned Oxigraph image. On the separate server,
repeat steps 2–4 with its own hostname, then create stopped containers:

```sh
dc build axum_gateway
dc create oxigraph_db
DB_CONTAINER=$(dc ps -a -q oxigraph_db)
test -n "$DB_CONTAINER"
gzip -t /absolute/path/graph-backup.tar.gz
sudo docker run --rm -i --network none --volumes-from "$DB_CONTAINER" \
    alpine:3 tar -C /data -xzf - < /absolute/path/graph-backup.tar.gz
dc up -d
```

Use an **empty database volume** for this procedure; do not unpack RocksDB files
over an existing database or while Oxigraph is running. Run the collection and
browser checks again. Restoring production replaces its data, so arrange a
maintenance window and preserve the current database before doing so.

## 10. Deploy updates and recover

1. Record the current release/config and take a database backup.
2. Keep a copy of the old source release. Extract the new archive to a staging
   directory and review it before replacing source files in `/opt/meshgraph`.
   Retain `.env.cloud` and the selected deployment mode.
3. Run `dc config --quiet`, then `dc build axum_gateway`. The old gateway stays
   running during the build. If it fails, production continues using its old image.
4. Run `dc up -d` and repeat step 7. Do not pull a new database version as part of
   a routine app update.
5. To roll back app code, restore the previous source release, rebuild the
   gateway, and run `dc up -d`. A database downgrade needs the matching backup
   and its tested image; app rollback does not undo database changes.

Database data survives `dc down` and application-directory replacement. **Do not
run `dc down --volumes`, `docker volume prune`, or delete `meshgraph_graph_data`.**
Do not rename the Compose project; that selects a different database volume.

Updating bundled `data/jpl-history/history.ttl` does not replace a populated
history graph. To deliberately apply an updated full JPL snapshot, first use
the README's importer workflow and review its report, take a database backup,
and deploy any updated image assets. Then replace **only the JPL history graph**
through an internal temporary container:

```sh
dc run --rm --no-deps --entrypoint curl graph_seed \
    --fail --silent --show-error --max-time 120 --request PUT \
    --header 'Content-Type: text/turtle' \
    --data-binary @/seed/jpl-history/history.ttl \
    --url-query 'graph=http://example.org/meshgraph/graphs/jpl-history' \
    http://oxigraph_db:7878/store
```

This replaces the JPL snapshot and retains the citation graph. The importer
expects a full snapshot, not a file containing additions only. Repeat the history
checks afterward. Do not expose the database publicly or force-seed both graphs
to perform a JPL-only update.

## Troubleshooting

| Symptom | Check / action |
| --- | --- |
| 80/443 already allocated | Use existing-proxy mode; retain the server's other websites. |
| Gateway loopback port allocated | Change `GATEWAY_PORT` and update the proxy upstream. |
| Certificate issuance fails | Check DNS A/AAAA, public reachability of 80/443, and Caddy logs. |
| Proxy returns 502 | Check `dc ps -a`, gateway health/logs, and the upstream address. |
| Health passes but graph API fails | Check Oxigraph and seed logs; health alone does not query the database. |
| Seed exits nonzero | Check database readiness, readable `data/` files, and LF line endings in `seed-graphs.sh`. |
| No images or blank scene | Check browser console/network and same-origin `/public/` requests; ensure bundled vendor files are present. |
| Disk fills | Check `df -h` and `sudo docker system df`; retain database volumes and rollback images when clearing build cache. |
| App contains only bundled data | Initial deployment creates a fresh volume; existing local data requires a separate migration. |

## Release preparation and references

To regenerate the handoff archive from the development repository, run
`python scripts/package-deployment.py`. Output is `releases/meshgraph-vps.zip`
with a SHA-256 checksum file next to it. Give the archive and this guide to the
tech person; share server credentials through the institution's usual channel.

Configuration references:
[Docker Compose service options](https://docs.docker.com/reference/compose-file/services/),
[shared Compose definitions](https://docs.docker.com/compose/how-tos/multiple-compose-files/extends/),
[Docker volume persistence](https://docs.docker.com/engine/storage/volumes/), and
[Nginx reverse proxy settings](https://nginx.org/en/docs/http/ngx_http_proxy_module.html).
